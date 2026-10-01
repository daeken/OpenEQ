use super::*;
use std::{rc::Rc, sync::Mutex, thread, time::Instant};

fn xmi(events: &[u8]) -> Vec<u8> {
    let mut evnt = b"EVNT".to_vec();
    evnt.extend((events.len() as u32).to_be_bytes());
    evnt.extend(events);
    if events.len() % 2 == 1 {
        evnt.push(0);
    }
    let mut form = b"FORM".to_vec();
    form.extend((4 + evnt.len() as u32).to_be_bytes());
    form.extend(b"XMID");
    form.extend(evnt);
    let mut bytes = b"FORM".to_vec();
    bytes.extend(14_u32.to_be_bytes());
    bytes.extend(b"XDIRINFO");
    bytes.extend(2_u32.to_be_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(b"CAT ");
    bytes.extend((4 + form.len() as u32).to_be_bytes());
    bytes.extend(b"XMID");
    bytes.extend(form);
    bytes
}

fn prepared(events: &[u8]) -> (XmiScheduler, u64) {
    let sequence = XmiFile::parse(&xmi(events)).unwrap().sequences.remove(0);
    let clock = SampleClock::miles_default(SAMPLE_RATE).unwrap();
    let end = clock.frame_at_tick(sequence.end_tick).unwrap();
    (XmiScheduler::new(Arc::new(sequence), clock).unwrap(), end)
}

#[derive(Default)]
struct Probe {
    messages: Mutex<Vec<(usize, u8, u8, u8)>>,
    renders: Mutex<Vec<(usize, usize)>>,
    owner: Mutex<Option<thread::ThreadId>>,
    dropped: AtomicBool,
}

// !Send, just like the native adapter. Tests prove creation and disposal occur
// on the worker even though the factory itself must cross a thread boundary.
struct FakeSynth {
    probe: Arc<Probe>,
    frame: usize,
    value: f32,
    fail_render_at: Option<usize>,
    fail_midi_at: Option<usize>,
    _thread_local: Rc<()>,
}
impl FakeSynth {
    fn new(probe: Arc<Probe>) -> Self {
        *probe.owner.lock().unwrap() = Some(thread::current().id());
        Self {
            probe,
            frame: 0,
            value: 0.0,
            fail_render_at: None,
            fail_midi_at: None,
            _thread_local: Rc::new(()),
        }
    }
}
impl RenderSynth for FakeSynth {
    fn midi_event(&mut self, status: u8, data1: u8, data2: u8) -> Result<()> {
        let mut messages = self.probe.messages.lock().unwrap();
        let call = messages.len();
        messages.push((self.frame, status, data1, data2));
        ensure!(self.fail_midi_at != Some(call), "injected MIDI failure");
        match status & 0xf0 {
            0x90 => self.value = f32::from(data1),
            0x80 => self.value = 0.0,
            _ => {}
        }
        Ok(())
    }
    fn render(&mut self, samples: &mut [f32]) -> Result<()> {
        assert!(samples.len().is_multiple_of(2));
        let frames = samples.len() / 2;
        assert!(frames > 0 && frames <= MAX_RENDER_FRAMES);
        let mut renders = self.probe.renders.lock().unwrap();
        let call = renders.len();
        renders.push((self.frame, frames));
        for frame in samples.chunks_exact_mut(2) {
            frame.copy_from_slice(&[self.value, -self.value]);
        }
        ensure!(self.fail_render_at != Some(call), "injected render failure");
        self.frame += frames;
        Ok(())
    }
}
impl Drop for FakeSynth {
    fn drop(&mut self) {
        assert_eq!(
            *self.probe.owner.lock().unwrap(),
            Some(thread::current().id())
        );
        self.probe.dropped.store(true, Ordering::Release);
    }
}

fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "worker did not finish bounded work"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

fn wait_for_pool(workers: &AtomicUsize) {
    wait_until(|| workers.load(Ordering::Acquire) == 0);
}

fn assert_silent_then_ended(source: &mut MusicStream) {
    wait_until(|| match source.next() {
        None => true,
        Some(sample) => {
            assert_eq!(sample, 0.0, "failed partial PCM must not reach output");
            false
        }
    });
}

#[test]
fn invalid_selection_and_unsupported_events_never_construct_a_synth() {
    let workers = Arc::new(AtomicUsize::new(0));
    let constructed = Arc::new(AtomicUsize::new(0));
    for (bytes, ordinal) in [
        (vec![], 0),
        (xmi(&[0xff, 0x2f, 0]), 1),
        (xmi(&[0xb0, 109, 127, 0xff, 0x2f, 0]), 0),
    ] {
        let count = Arc::clone(&constructed);
        let result = stream_with(
            bytes,
            XmiSequenceOrdinal(ordinal),
            Arc::clone(&workers),
            move || {
                count.fetch_add(1, Ordering::Relaxed);
                Ok(FakeSynth::new(Arc::default()))
            },
        );
        assert!(result.is_err());
        assert_eq!(workers.load(Ordering::Acquire), 0);
    }
    assert_eq!(constructed.load(Ordering::Acquire), 0);
}

#[test]
fn pcm_splits_at_events_preserve_source_order_and_exact_release_frames() {
    let (mut schedule, end) = prepared(&[
        0xc0, 0, // program at zero
        0x90, 60, 100, 1, // key 60, duration one tick
        1, 0x90, 61, 100, 0, // zero-duration key 61 at tick one
        0xb0, 10, 64, // same-tick controller follows the note
        2, 0xff, 0x2f, 0, // end tick three
    ]);
    let probe = Arc::new(Probe::default());
    let mut synth = FakeSynth::new(Arc::clone(&probe));
    let mut blocks = Vec::new();
    render(&mut schedule, &mut synth, end, |block| {
        blocks.push(block);
        true
    })
    .unwrap();
    assert_eq!(
        *probe.messages.lock().unwrap(),
        [
            (0, 0xc0, 0, 0),
            (0, 0x90, 60, 100),
            (400, 0x80, 60, 0),
            (400, 0x90, 61, 100),
            (400, 0xb0, 10, 64),
            (800, 0x80, 61, 0),
        ]
    );
    assert!(
        blocks
            .iter()
            .all(|block| block.len() <= MAX_RENDER_FRAMES * 2)
    );
    assert!(
        blocks[..blocks.len() - 1]
            .iter()
            .all(|block| block.len() == MAX_RENDER_FRAMES * 2)
    );
    let samples: Vec<_> = blocks.into_iter().flatten().collect();
    assert_eq!(
        samples.len(),
        (1200 + RELEASE_SECONDS as usize * SAMPLE_RATE as usize) * 2
    );
    for (frame, stereo) in samples.chunks_exact(2).enumerate() {
        let expected = if frame < 400 {
            60.0
        } else if frame < 800 {
            61.0
        } else {
            0.0
        };
        assert_eq!(stereo, [expected, -expected], "frame {frame}");
    }
    assert!(schedule.is_finished());
}

#[test]
fn receiver_cancellation_stops_after_one_block_and_discards_future_events() {
    let (mut schedule, end) = prepared(&[0x90, 60, 100, 120, 120, 0xc0, 4, 0xff, 0x2f, 0]);
    let probe = Arc::new(Probe::default());
    let mut synth = FakeSynth::new(Arc::clone(&probe));
    let mut publications = 0;
    render(&mut schedule, &mut synth, end, |block| {
        publications += 1;
        assert_eq!(block.len(), MAX_RENDER_FRAMES * 2);
        false
    })
    .unwrap();
    assert_eq!(publications, 1);
    assert_eq!(synth.frame, MAX_RENDER_FRAMES);
    assert_eq!(
        *probe.messages.lock().unwrap(),
        [(0, 0x90, 60, 100), (MAX_RENDER_FRAMES, 0x80, 60, 0)]
    );
}

#[test]
fn finite_loop_renders_through_execution_eot_and_then_the_release_tail() {
    let (mut schedule, source_end) = prepared(&[
        0xb0, 116, 2, 0x90, 60, 100, 1, 2, 0xb0, 117, 127, 0xff, 0x2f, 0,
    ]);
    assert_eq!(source_end, 800);
    assert_eq!(schedule.known_end_frame(), None);
    let probe = Arc::new(Probe::default());
    let mut synth = FakeSynth::new(Arc::clone(&probe));
    let mut total = 0;
    render(&mut schedule, &mut synth, u64::from(SAMPLE_RATE), |block| {
        total += block.len() / 2;
        true
    })
    .unwrap();
    assert_eq!(
        total,
        4000 + RELEASE_SECONDS as usize * SAMPLE_RATE as usize
    );
    assert_eq!(
        *probe.messages.lock().unwrap(),
        (0..5)
            .flat_map(|n| [(n * 800, 0x90, 60, 100), (n * 800 + 400, 0x80, 60, 0),])
            .collect::<Vec<_>>()
    );
    assert!(schedule.is_finished());
}

#[test]
fn infinite_loop_continues_after_source_eot_and_policy_cutoff_releases_notes_and_pedal() {
    let (mut schedule, source_end) = prepared(&[
        0xb0, 64, 127, 0xb0, 116, 0, 0x90, 60, 100, 100, 2, 0xb0, 117, 127, 0xff, 0x2f, 0,
    ]);
    assert_eq!(source_end, 800);
    let probe = Arc::new(Probe::default());
    let mut synth = FakeSynth::new(Arc::clone(&probe));
    let mut total = 0;
    render(&mut schedule, &mut synth, 1000, |block| {
        total += block.len() / 2;
        true
    })
    .unwrap();
    assert_eq!(
        *probe.messages.lock().unwrap(),
        [
            (0, 0xb0, 64, 127),
            (0, 0x90, 60, 100),
            (800, 0x90, 60, 100),
            (1000, 0x80, 60, 0),
            (1000, 0x80, 60, 0),
            (1000, 0xb0, 64, 0),
        ]
    );
    assert_eq!(
        total,
        1000 + RELEASE_SECONDS as usize * SAMPLE_RATE as usize
    );
    assert!(schedule.is_finished());
    assert!(schedule.cancel().is_empty());
}

#[test]
fn control_only_loop_is_bounded_and_cleans_notes_without_publishing_partial_audio() {
    let (mut schedule, _) = prepared(&[
        0x90, 60, 100, 100, 0xb0, 64, 127, 0xb0, 116, 0, 0xb0, 117, 127, 0xff, 0x2f, 0,
    ]);
    let probe = Arc::new(Probe::default());
    let mut synth = FakeSynth::new(Arc::clone(&probe));
    let mut publications = 0;
    let error = render(&mut schedule, &mut synth, u64::from(SAMPLE_RATE), |_| {
        publications += 1;
        true
    })
    .unwrap_err();
    assert_eq!(
        error
            .downcast_ref::<super::super::schedule::ScheduleError>()
            .unwrap()
            .issue,
        super::super::schedule::ScheduleIssue::SameTickWorkLimit
    );
    assert_eq!(publications, 0);
    assert_eq!(synth.frame, 0);
    assert_eq!(
        *probe.messages.lock().unwrap(),
        [
            (0, 0x90, 60, 100),
            (0, 0xb0, 64, 127),
            (0, 0x80, 60, 0),
            (0, 0xb0, 64, 0),
        ]
    );
    assert!(schedule.is_finished());
}

#[test]
fn per_block_work_limit_survives_advancing_ticks() {
    let mut bytes = vec![0xb0, 116, 0];
    for _ in 0..16_384 {
        bytes.extend_from_slice(&[0xff, 1, 0]);
    }
    bytes.extend_from_slice(&[1, 0xb0, 117, 127, 0xff, 0x2f, 0]);
    let (mut schedule, _) = prepared(&bytes);
    let probe = Arc::new(Probe::default());
    let mut synth = FakeSynth::new(Arc::clone(&probe));
    let error = render(&mut schedule, &mut synth, u64::from(SAMPLE_RATE), |_| {
        panic!("bounded work failure must discard its partial PCM block")
    })
    .unwrap_err();
    assert!(error.to_string().contains("event-work limit"));
    assert!(synth.frame > 0 && synth.frame < MAX_RENDER_FRAMES);
    assert!(schedule.is_finished());
}

#[test]
fn cutoff_attempts_all_cleanup_messages_even_if_one_is_rejected_by_synth() {
    let (mut schedule, _) = prepared(&[
        0xb0, 64, 127, 0xb0, 116, 0, 0x90, 60, 100, 100, 2, 0xb0, 117, 127, 0xff, 0x2f, 0,
    ]);
    let probe = Arc::new(Probe::default());
    let mut synth = FakeSynth::new(Arc::clone(&probe));
    synth.fail_midi_at = Some(3); // First cutoff release after pedal and two notes.
    let error = render(&mut schedule, &mut synth, 1000, |_| true).unwrap_err();
    assert!(error.to_string().contains("injected MIDI failure"));
    assert_eq!(
        probe.messages.lock().unwrap()[3..],
        [
            (1000, 0x80, 60, 0),
            (1000, 0x80, 60, 0),
            (1000, 0xb0, 64, 0),
        ]
    );
    assert!(schedule.is_finished());
    assert!(schedule.cancel().is_empty());
}

#[test]
fn looping_receiver_cancellation_stops_after_one_block_and_metadata_has_no_false_endpoint() {
    use rodio::Source;
    let bytes = [
        0xb0, 116, 0, 0x90, 60, 100, 100, 2, 0xb0, 117, 127, 0xff, 0x2f, 0,
    ];
    let (mut schedule, _) = prepared(&bytes);
    let probe = Arc::new(Probe::default());
    let mut synth = FakeSynth::new(Arc::clone(&probe));
    let mut publications = 0;
    render(
        &mut schedule,
        &mut synth,
        MAX_SECONDS * u64::from(SAMPLE_RATE),
        |_| {
            publications += 1;
            false
        },
    )
    .unwrap();
    assert_eq!(publications, 1);
    assert_eq!(synth.frame, MAX_RENDER_FRAMES);
    let messages = probe.messages.lock().unwrap();
    assert_eq!(messages.len(), 12);
    assert_eq!(
        messages[..6],
        (0..6).map(|n| (n * 800, 0x90, 60, 100)).collect::<Vec<_>>()
    );
    assert_eq!(messages[6..], [(MAX_RENDER_FRAMES, 0x80, 60, 0); 6]);
    assert!(schedule.is_finished());
    drop(messages);

    let workers = Arc::new(AtomicUsize::new(0));
    let source = stream_with(
        xmi(&bytes),
        XmiSequenceOrdinal(0),
        Arc::clone(&workers),
        || Ok(FakeSynth::new(Arc::default())),
    )
    .unwrap();
    assert_eq!(source.total_duration(), None);
    drop(source);
    wait_for_pool(&workers);
}

#[test]
fn initialization_and_partial_block_render_failures_disconnect_and_report_failure() {
    let workers = Arc::new(AtomicUsize::new(0));
    let mut source = stream_with::<FakeSynth>(
        xmi(&[0xff, 0x2f, 0]),
        XmiSequenceOrdinal(0),
        Arc::clone(&workers),
        || anyhow::bail!("injected initialization failure"),
    )
    .unwrap();
    let failure = source.failure();
    wait_for_pool(&workers);
    assert!(failure.load(Ordering::Acquire));
    assert_silent_then_ended(&mut source);
    drop(source);

    let probe = Arc::new(Probe::default());
    let worker_probe = Arc::clone(&probe);
    let mut source = stream_with(
        xmi(&[0x90, 60, 100, 1, 120, 0xff, 0x2f, 0]),
        XmiSequenceOrdinal(0),
        Arc::clone(&workers),
        move || {
            let mut synth = FakeSynth::new(worker_probe);
            synth.fail_render_at = Some(1);
            Ok(synth)
        },
    )
    .unwrap();
    let failure = source.failure();
    wait_for_pool(&workers);
    assert!(failure.load(Ordering::Acquire));
    assert!(probe.dropped.load(Ordering::Acquire));
    // A 400-frame prefix existed before the second render failed, but no
    // partial block from that failure may leak into the consumer queue.
    assert_silent_then_ended(&mut source);
    assert_eq!(*probe.renders.lock().unwrap(), [(0, 400), (400, 3696)]);
}

#[test]
fn full_queue_drop_unblocks_worker_and_releases_its_slot_on_owning_thread() {
    let workers = Arc::new(AtomicUsize::new(0));
    let probe = Arc::new(Probe::default());
    let worker_probe = Arc::clone(&probe);
    let source = stream_with(
        xmi(&[120, 0xff, 0x2f, 0]),
        XmiSequenceOrdinal(0),
        Arc::clone(&workers),
        move || Ok(FakeSynth::new(worker_probe)),
    )
    .unwrap();
    let failure = source.failure();
    wait_until(|| probe.renders.lock().unwrap().len() == 9);
    // Eight full queued blocks plus one blocked producer block, no consumer.
    assert_eq!(workers.load(Ordering::Acquire), 1);
    assert!(!probe.dropped.load(Ordering::Acquire));
    drop(source);
    wait_for_pool(&workers);
    assert!(probe.dropped.load(Ordering::Acquire));
    assert!(!failure.load(Ordering::Acquire));
    assert_eq!(probe.renders.lock().unwrap().len(), 9);
    assert_ne!(*probe.owner.lock().unwrap(), Some(thread::current().id()));
}

#[test]
fn startup_is_async_and_two_initializing_workers_apply_backpressure() {
    let workers = Arc::new(AtomicUsize::new(0));
    let mut held = Vec::new();
    let mut probes = Vec::new();
    for _ in 0..2 {
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let (returned_tx, returned_rx) = mpsc::sync_channel(1);
        let pool = Arc::clone(&workers);
        let probe = Arc::new(Probe::default());
        probes.push(Arc::clone(&probe));
        // The watchdog can fail without hanging if startup becomes synchronous.
        thread::spawn(move || {
            let source = stream_with(
                xmi(&[120, 0xff, 0x2f, 0]),
                XmiSequenceOrdinal(0),
                pool,
                move || {
                    entered_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    Ok(FakeSynth::new(probe))
                },
            );
            let _ = returned_tx.send(source);
        });
        let source = returned_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("startup must return before factory completes")
            .unwrap();
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        held.push((source, release_tx));
    }
    assert_eq!(workers.load(Ordering::Acquire), 2);
    let third = stream_with(
        xmi(&[0xff, 0x2f, 0]),
        XmiSequenceOrdinal(0),
        Arc::clone(&workers),
        || Ok(FakeSynth::new(Arc::default())),
    );
    assert!(matches!(third, Err(error) if error.is::<Busy>()));
    for (source, release) in held {
        // Cancel before the factory completes. It may finish initialization,
        // but cannot publish to a replacement consumer or survive disposal.
        drop(source);
        release.send(()).unwrap();
    }
    wait_for_pool(&workers);
    assert!(
        probes
            .iter()
            .all(|probe| probe.dropped.load(Ordering::Acquire))
    );
    assert!(
        probes
            .iter()
            .all(|probe| probe.renders.lock().unwrap().len() == 1)
    );
}

#[test]
#[ignore = "requires original GFay XMI and macOS DLSSynth; silent offline consumption only"]
#[cfg(target_os = "macos")]
fn original_gfay_sequences_synthesize_and_cancel_without_output() {
    use rodio::Source;
    let base = openeq_assets::loader::default_client_dir().unwrap();
    let bytes = std::fs::read(base.join("gfaydark.xmi")).unwrap();
    let workers = WORKERS.get_or_init(Arc::default);
    for ordinal in [0, 2, 5] {
        wait_for_pool(workers);
        let mut source = stream(bytes.clone(), XmiSequenceOrdinal(ordinal)).unwrap();
        assert_eq!(source.channels().get(), 2);
        assert_eq!(source.sample_rate().get(), SAMPLE_RATE);
        let mut peak = 0f32;
        let deadline = Instant::now() + Duration::from_secs(10);
        while peak < 0.001 && Instant::now() < deadline {
            for sample in source.by_ref().take(4096) {
                assert!(sample.is_finite());
                peak = peak.max(sample.abs());
            }
            assert!(!source.failure().load(Ordering::Acquire));
            thread::sleep(Duration::from_millis(1));
        }
        assert!(peak >= 0.001, "sequence {ordinal} must generate music");
        // Give the small queue time to fill, then drop without output playback.
        thread::sleep(Duration::from_millis(50));
        drop(source);
        wait_for_pool(workers);
    }
    assert!(stream(bytes, XmiSequenceOrdinal(600)).is_err());
    wait_for_pool(workers);
}
