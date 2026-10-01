//! Synthetic results and original trace digests from the unmodified native
//! Miles walker; see docs/XMI_NATIVE_LOOPS.md. No synthesizer/device is used.
use super::{
    tests::{cc, drain, midis, note, scheduler, seq},
    *,
};

fn notes(events: &[ScheduledEvent], key: u8) -> Vec<u64> {
    midis(events)
        .into_iter()
        .filter_map(|(tick, status, k, _)| (status == 0x90 && k == key).then_some(tick))
        .collect()
}

#[test]
fn all_positive_native_counts_restart_the_marker_and_consume_controls() {
    for count in 1..=127 {
        let mut s = scheduler(
            &[
                (0, cc(0, 116, count)),
                (0, note(60, 1)),
                (2, cc(0, 117, 127)),
            ],
            2,
        );
        assert_eq!(s.known_end_frame(), None);
        let events = drain(&mut s, 7);
        let passes = if count == 1 { 1 } else { u64::from(count) + 3 };
        assert_eq!(
            notes(&events, 60),
            (0..passes).map(|n| n * 2).collect::<Vec<_>>()
        );
        assert_eq!(events.last().unwrap().tick, passes * 2);
        assert_eq!(midis(&events).len(), passes as usize * 2);
        let markers = events
            .iter()
            .filter(|event| {
                matches!(
                    event.kind,
                    ScheduledKind::Source {
                        event_index: 0,
                        message: None,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(markers, passes as usize);
        let counters = s.loops.map(|slot| slot.map(|slot| slot.remaining));
        assert_eq!(
            counters,
            if count == 1 {
                [None; 4]
            } else {
                [Some(count - 1), Some(count - 1), Some(count - 1), None]
            }
        );
    }
}

#[test]
fn native_nested_loops_share_four_slots_across_channels() {
    let mut s = scheduler(
        &[
            (0, cc(0, 116, 2)),
            (0, note(60, 1)),
            (0, cc(1, 116, 3)),
            (0, note(61, 1)),
            (2, cc(5, 117, 127)),
            (2, note(62, 1)),
            (5, cc(7, 117, 127)),
        ],
        5,
    );
    let events = drain(&mut s, 1);
    assert_eq!(notes(&events, 60), [0]);
    assert_eq!(notes(&events, 61), [0, 2, 4, 6, 8, 13, 15, 17]);
    assert_eq!(notes(&events, 62), [10, 19]);
    assert_eq!(events.last().unwrap().tick, 22);
    assert_eq!(
        s.loops.map(|slot| slot.map(|slot| slot.remaining)),
        [Some(2), Some(2), None, None]
    );

    let mut values = (1..=5).map(|n| (0, cc(n, 116, n))).collect::<Vec<_>>();
    values.extend((0..4).map(|_| (2, cc(9, 117, 0))));
    let mut s = scheduler(&values, 2);
    assert_eq!(s.next_batch(5).unwrap().len(), 5);
    assert_eq!(
        s.loops
            .map(|slot| slot.map(|slot| (slot.start, slot.remaining))),
        [Some((0, 1)), Some((1, 2)), Some((2, 3)), Some((3, 4))]
    );
    assert!(midis(&drain(&mut s, 512)).is_empty());
    assert!(s.loops.iter().all(Option::is_none));
}

#[test]
fn native_next_threshold_and_safe_unmatched_break() {
    for value in [64, 127] {
        let events = drain(
            &mut scheduler(
                &[(0, cc(0, 116, 2)), (0, note(60, 1)), (2, cc(0, 117, value))],
                2,
            ),
            512,
        );
        assert_eq!(notes(&events, 60), [0, 2, 4, 6, 8]);
        let unmatched = drain(&mut scheduler(&[(0, cc(0, 117, value))], 0), 1);
        assert!(midis(&unmatched).is_empty());
    }
    for value in [0, 63] {
        let events = drain(
            &mut scheduler(
                &[(0, cc(0, 116, 0)), (0, note(60, 1)), (2, cc(0, 117, value))],
                2,
            ),
            512,
        );
        assert_eq!(notes(&events, 60), [0]);
        let mut s = scheduler(&[(0, cc(0, 117, value))], 0);
        let error = s.next_event().unwrap_err();
        assert_eq!(
            error,
            ScheduleError {
                event_index: Some(0),
                issue: ScheduleIssue::UnmatchedLoopBreak
            }
        );
        assert_eq!(s.next_event().unwrap_err(), error);
        assert_eq!(s.next_frame().unwrap_err(), error);
        assert!(s.cancel().is_empty());
        assert!(s.next_event().unwrap().is_none());
    }
}

#[test]
fn marker_delay_is_skipped_on_restart_but_body_delay_is_repeated() {
    let before = drain(
        &mut scheduler(
            &[(3, cc(0, 116, 2)), (3, note(60, 1)), (5, cc(0, 117, 127))],
            5,
        ),
        512,
    );
    assert_eq!(notes(&before, 60), [3, 5, 7, 9, 11]);
    assert_eq!(before.last().unwrap().tick, 13);
    let after = drain(
        &mut scheduler(
            &[(0, cc(0, 116, 2)), (3, note(60, 1)), (5, cc(0, 117, 127))],
            5,
        ),
        512,
    );
    assert_eq!(notes(&after, 60), [3, 8, 13, 18, 23]);
    assert_eq!(after.last().unwrap().tick, 25);
}

#[test]
fn loop_releases_keep_execution_time_slot_order_and_unique_note_identity() {
    let values = [(0, cc(0, 116, 2)), (0, note(60, 5)), (2, cc(0, 117, 127))];
    let source = seq(&values, 10);
    let original = (*source).clone();
    let mut s = XmiScheduler::new(
        Arc::clone(&source),
        SampleClock::miles_default(48000).unwrap(),
    )
    .unwrap();
    let events = drain(&mut s, 512);
    assert!(Arc::ptr_eq(&source, &s.sequence));
    assert_eq!(*source, original);
    assert_eq!(notes(&events, 60), [0, 2, 4, 6, 8]);
    let releases = events
        .iter()
        .filter_map(|event| match event.kind {
            ScheduledKind::Release { note, .. } => Some((event.tick, note.occurrence, note.slot)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        releases,
        [(5, 0, 0), (7, 1, 1), (9, 2, 2), (11, 3, 0), (13, 4, 1)]
    );
    assert_eq!(events.last().unwrap().tick, 18);
    for chunk in [1, 2, 3, 7, 511] {
        assert_eq!(drain(&mut scheduler(&values, 10), chunk), events);
    }
    // Release coincident with NEXT must precede the restarted same-key note.
    let events = drain(
        &mut scheduler(
            &[(0, cc(0, 116, 2)), (0, note(60, 2)), (2, cc(0, 117, 127))],
            2,
        ),
        512,
    );
    assert_eq!(
        &midis(&events)[..3],
        [(0, 0x90, 60, 90), (2, 0x80, 60, 0), (2, 0x90, 60, 90)]
    );
}

#[test]
fn infinite_loops_are_lazy_cancellable_and_same_tick_work_limit_survives_batches() {
    let mut s = scheduler(
        &[(0, cc(0, 116, 0)), (0, note(60, 1)), (2, cc(0, 117, 127))],
        2,
    );
    let mut onsets = Vec::new();
    while onsets.len() < 1000 {
        let event = s.next_event().unwrap().unwrap();
        if matches!(event.kind, ScheduledKind::Source { note: Some(_), .. }) {
            onsets.push(event.tick);
        }
    }
    assert_eq!(onsets, (0..1000).map(|n| n * 2).collect::<Vec<_>>());
    assert!(!s.is_finished());
    assert_eq!(
        s.loops.map(|slot| slot.map(|slot| slot.remaining)),
        [Some(0); 4]
    );
    assert_eq!(s.cancel().len(), 1);
    assert!(s.cancel().is_empty());
    assert!(s.next_frame().unwrap().is_none());

    let values = [
        (0, cc(0, 64, 127)),
        (0, note(60, 100)),
        (0, cc(0, 116, 0)),
        (0, cc(0, 117, 127)),
    ];
    let mut s = scheduler(&values, 0);
    for _ in 0..MAX_SOURCE_EVENTS_PER_TICK / MAX_BATCH_EVENTS {
        let events = s.next_batch(MAX_BATCH_EVENTS).unwrap();
        assert_eq!(events.len(), MAX_BATCH_EVENTS);
        assert!(events.iter().all(|event| event.tick == 0));
    }
    let error = s.next_event().unwrap_err();
    assert_eq!(error.issue, ScheduleIssue::SameTickWorkLimit);
    assert_eq!(s.next_batch(1).unwrap_err(), error);
    assert_eq!(
        s.cancel(),
        [
            MidiMessage {
                status: 0x80,
                data1: 60,
                data2: 0
            },
            MidiMessage {
                status: 0xb0,
                data1: 64,
                data2: 0
            }
        ]
    );
    assert!(s.cancel().is_empty());
}

#[test]
fn repeats_enforce_runtime_note_capacity_and_checked_clocks() {
    let source = seq(
        &[
            (0, cc(0, 116, 127)),
            (0, note(60, 100)),
            (1, cc(0, 117, 127)),
        ],
        1,
    );
    let clock = SampleClock::new(1, 1).unwrap();
    assert!(preflight(&source, clock).is_supported());
    let mut s = XmiScheduler::new(source, clock).unwrap();
    let error = loop {
        match s.next_event() {
            Ok(Some(_)) => {}
            result => break result.unwrap_err(),
        }
    };
    assert_eq!(error.issue, ScheduleIssue::ActiveNoteLimit);
    assert_eq!(s.releases.len(), 32);
    assert_eq!(s.cancel().len(), 32);
    assert!(s.releases.is_empty());

    let source = seq(
        &[(0, cc(0, 116, 0)), (u64::MAX / 2, cc(0, 117, 127))],
        u64::MAX / 2,
    );
    let mut s = XmiScheduler::new(source, clock).unwrap();
    for _ in 0..4 {
        s.next_event().unwrap().unwrap();
    }
    assert_eq!(
        s.next_event().unwrap_err().issue,
        ScheduleIssue::TickOverflow
    );
    assert!(s.cancel().is_empty());

    let tick = u64::MAX / 2;
    let source = seq(&[(0, cc(0, 116, 0)), (tick, cc(0, 117, 127))], tick);
    let mut s = XmiScheduler::new(source, SampleClock::new(2, 1).unwrap()).unwrap();
    for _ in 0..3 {
        s.next_event().unwrap().unwrap();
    }
    assert_eq!(
        s.next_frame().unwrap_err().issue,
        ScheduleIssue::FrameOverflow
    );
    assert_eq!(
        s.next_event().unwrap_err().issue,
        ScheduleIssue::FrameOverflow
    );
    assert!(s.cancel().is_empty());

    let mut s = scheduler(&[(0, note(60, 1))], 2);
    s.next_note_id = u64::MAX;
    assert_eq!(
        s.next_event().unwrap_err().issue,
        ScheduleIssue::IdentityOverflow
    );
    assert!(s.cancel().is_empty());
}

// FNV-1a over effective tick (u64 LE), status, data1, data2. Native C0/D0
// intercepts carry an unused extra byte; that byte is normalized to zero.
fn add_midi_digest(hash: &mut u64, event: ScheduledEvent) -> bool {
    let Some(m) = event.midi_message() else {
        return false;
    };
    for byte in event
        .tick
        .to_le_bytes()
        .into_iter()
        .chain([m.status, m.data1, m.data2])
    {
        *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    true
}

#[test]
#[ignore = "requires original XMI files in EQ_DIR; silent comparison with native trace digests"]
fn original_loop_sequences_match_native_midi_and_execution_ticks() {
    let base = std::path::PathBuf::from(std::env::var_os("EQ_DIR").expect("set EQ_DIR"));
    for (name, ordinal, finite) in [
        ("thurgadinb.xmi", 0, true),
        ("thurgadina.xmi", 5, true),
        ("templeveeshan.xmi", 0, false),
        ("thurgadina.xmi", 0, false),
    ] {
        let file =
            openeq_assets::audio::xmi::XmiFile::parse(&std::fs::read(base.join(name)).unwrap())
                .unwrap();
        let source = Arc::new(file.sequences[ordinal].clone());
        let original = (*source).clone();
        let mut s =
            XmiScheduler::new(Arc::clone(&source), SampleClock::new(1, 1).unwrap()).unwrap();
        let limit = if finite { 1_703_932 } else { 69_686 };
        let mut hash = 0xcbf29ce484222325;
        let mut midi_count = 0;
        let mut source_count = 0;
        let mut loops = 0;
        let mut next_ticks = Vec::new();
        let mut last_tick = 0;
        while s.next_frame().unwrap().is_some_and(|tick| tick <= limit) {
            let event = s.next_event().unwrap().unwrap();
            assert!(event.tick >= last_tick);
            last_tick = event.tick;
            midi_count += usize::from(add_midi_digest(&mut hash, event));
            if let ScheduledKind::Source { event_index, .. } = event.kind {
                let kind = &source.events[event_index].kind;
                source_count += usize::from(midi_message(kind).unwrap().is_some());
                if let XmiEventKind::Controller {
                    controller: 116 | 117,
                    ..
                } = kind
                {
                    loops += 1;
                }
                if let XmiEventKind::Controller {
                    controller: 117, ..
                } = kind
                {
                    next_ticks.push(event.tick);
                }
            }
        }
        assert_eq!(last_tick, limit);
        if finite {
            assert!(s.is_finished());
            assert_eq!(
                (midi_count, source_count, loops, hash),
                (261_313, 147_645, 260, 0x5111761a887c4982),
                "{name}/{ordinal}"
            );
            assert_eq!(
                next_ticks,
                (0..130).map(|n| 15_185 + n * 13_091).collect::<Vec<_>>()
            );
        } else {
            assert!(!s.is_finished());
            assert_eq!(
                (midi_count, source_count, loops, hash),
                (14_195, 7_402, 13, 0xfca1b8a525ce5142),
                "{name}/{ordinal}"
            );
            assert_eq!(next_ticks, [16971, 27514, 38057, 48600, 59143, 69686]);
        }
        assert_eq!(*source, original);
        assert!(s.cancel().len() <= MAX_ACTIVE_NOTES + 16);
    }
}
