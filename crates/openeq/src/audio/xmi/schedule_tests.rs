use super::*;
use openeq_assets::audio::xmi::{XmiEvent, XmiSequenceOrdinal};
pub(super) fn note(key: u8, duration: u32) -> XmiEventKind {
    XmiEventKind::NoteOn {
        channel: 0,
        key,
        velocity: 90,
        duration_ticks: duration,
    }
}
pub(super) fn cc(channel: u8, controller: u8, value: u8) -> XmiEventKind {
    XmiEventKind::Controller {
        channel,
        controller,
        value,
    }
}
pub(super) fn seq(values: &[(u64, XmiEventKind)], end: u64) -> Arc<XmiSequence> {
    let mut previous = 0;
    let mut events = values
        .iter()
        .enumerate()
        .map(|(index, (tick, kind))| {
            let event = XmiEvent {
                offset: index as u32,
                status_offset: index as u32,
                delay_ticks: tick.saturating_sub(previous),
                has_zero_delay_byte: false,
                tick: *tick,
                kind: kind.clone(),
            };
            previous = *tick;
            event
        })
        .collect::<Vec<_>>();
    events.push(XmiEvent {
        offset: events.len() as u32,
        status_offset: events.len() as u32,
        delay_ticks: end.saturating_sub(previous),
        has_zero_delay_byte: false,
        tick: end,
        kind: XmiEventKind::Meta {
            kind: 0x2f,
            payload: vec![],
        },
    });
    Arc::new(XmiSequence {
        ordinal: XmiSequenceOrdinal(0),
        form_offset: 0,
        evnt_payload_offset: 0,
        evnt_payload_len: 0,
        timbres: None,
        branches: None,
        events,
        end_tick: end,
        has_eot_padding: false,
    })
}
pub(super) fn scheduler(values: &[(u64, XmiEventKind)], end: u64) -> XmiScheduler {
    XmiScheduler::new(seq(values, end), SampleClock::miles_default(48000).unwrap()).unwrap()
}
pub(super) fn drain(s: &mut XmiScheduler, chunk: usize) -> Vec<ScheduledEvent> {
    let mut output = Vec::new();
    while !s.is_finished() {
        output.extend(s.next_batch(chunk).unwrap());
    }
    output
}
pub(super) fn midis(events: &[ScheduledEvent]) -> Vec<(u64, u8, u8, u8)> {
    events
        .iter()
        .filter_map(|event| {
            event
                .midi_message()
                .map(|message| (event.tick, message.status, message.data1, message.data2))
        })
        .collect()
}

#[test]
fn rational_clock_has_no_drift_and_checked_extremes() {
    let rate441 = SampleClock::miles_default(44100).unwrap();
    let rate48 = SampleClock::miles_default(48000).unwrap();
    assert_eq!(
        (
            rate441.frame_at_tick(1).unwrap(),
            rate441.frame_at_tick(2).unwrap()
        ),
        (367, 735)
    );
    assert_eq!(rate48.frame_at_tick(1).unwrap(), 400);
    assert_eq!(
        rate441.frame_at_tick(120 * 60 * 60 * 24).unwrap(),
        44100 * 60 * 60 * 24
    );
    let mut sum = 0;
    let mut previous = 0;
    for tick in 1..=39431 {
        let frame = rate441.frame_at_tick(tick).unwrap();
        sum += frame - previous;
        previous = frame;
    }
    assert_eq!(sum, 39431u64 * 44100 / 120);
    assert!(SampleClock::new(0, 120).is_err());
    assert!(SampleClock::new(48000, 0).is_err());
    assert!(rate48.frame_at_tick(u64::MAX).is_err());
    assert_eq!(
        SampleClock::new(1, 1)
            .unwrap()
            .frame_at_tick(u64::MAX)
            .unwrap(),
        u64::MAX
    );
    assert_eq!(
        SampleClock::new(1, 120).unwrap().frame_at_tick(1).unwrap(),
        0
    );
}

#[test]
fn releases_precede_source_ties_and_expiry_uses_slot_order_not_note_age() {
    let mut s = scheduler(
        &[
            (0, note(60, 2)),
            (0, note(61, 4)),
            (2, note(62, 2)),
            (4, cc(0, 7, 100)),
        ],
        5,
    );
    let events = drain(&mut s, 1);
    assert_eq!(
        midis(&events),
        [
            (0, 0x90, 60, 90),
            (0, 0x90, 61, 90),
            (2, 0x80, 60, 0),
            (2, 0x90, 62, 90),
            (4, 0x80, 62, 0),
            (4, 0x80, 61, 0),
            (4, 0xb0, 7, 100)
        ]
    );
    let releases = events
        .iter()
        .filter_map(|event| {
            if let ScheduledKind::Release { note, .. } = event.kind {
                Some((event.tick, note.slot, note.source_event))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(releases, [(2, 0, 0), (4, 0, 2), (4, 1, 1)]);
    assert!(events.iter().all(|event| event.frame == event.tick * 400));
    assert!(matches!(events.last().unwrap().kind, ScheduledKind::End));
    assert!(s.next_event().unwrap().is_none());
    assert_eq!(s.next_frame().unwrap(), None);
}

#[test]
fn zero_duration_waits_one_tick_and_overlap_keeps_unconditional_releases() {
    let mut s = scheduler(&[(0, note(60, 0)), (0, note(60, 3)), (1, note(60, 1))], 4);
    let events = drain(&mut s, 512);
    assert_eq!(
        midis(&events),
        [
            (0, 0x90, 60, 90),
            (0, 0x90, 60, 90),
            (1, 0x80, 60, 0),
            (1, 0x90, 60, 90),
            (2, 0x80, 60, 0),
            (3, 0x80, 60, 0)
        ]
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.kind, ScheduledKind::Source { note: Some(_), .. }))
            .count(),
        3
    );
    // No token-based suppression of an old key's off; native emits every slot's off.
}

#[test]
fn eot_and_cancel_release_slots_then_pedals_without_fabricated_all_sound_off() {
    let values = [
        (0, cc(3, 64, 127)),
        (0, cc(1, 64, 64)),
        (0, cc(2, 64, 63)),
        (0, note(60, 100)),
        (0, note(61, 0)),
    ];
    let mut s = scheduler(&values, 0);
    let events = drain(&mut s, 3);
    let cleanup = events
        .iter()
        .filter_map(|event| {
            if let ScheduledKind::Cleanup { message, .. } = event.kind {
                Some(message)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        cleanup,
        [
            MidiMessage {
                status: 0x80,
                data1: 60,
                data2: 0
            },
            MidiMessage {
                status: 0x80,
                data1: 61,
                data2: 0
            },
            MidiMessage {
                status: 0xb1,
                data1: 64,
                data2: 0
            },
            MidiMessage {
                status: 0xb3,
                data1: 64,
                data2: 0
            }
        ]
    );
    assert!(
        events
            .iter()
            .all(|event| event.tick == 0 && event.frame == 0)
    );
    let mut cancelled = scheduler(&values, 50);
    cancelled.next_batch(values.len()).unwrap();
    assert_eq!(cancelled.cancel(), cleanup);
    assert!(cancelled.cancel().is_empty());
    assert!(cancelled.next_event().unwrap().is_none());
    assert!(cancelled.next_frame().unwrap().is_none());
}

#[test]
fn source_metadata_and_channel_shapes_keep_same_tick_order() {
    let values = [
        (
            0,
            XmiEventKind::Meta {
                kind: 0x51,
                payload: vec![1, 2, 3],
            },
        ),
        (
            0,
            XmiEventKind::ProgramChange {
                channel: 4,
                program: 5,
            },
        ),
        (
            0,
            XmiEventKind::ChannelPressure {
                channel: 4,
                pressure: 6,
            },
        ),
        (
            0,
            XmiEventKind::PolyPressure {
                channel: 4,
                key: 60,
                pressure: 7,
            },
        ),
        (
            1,
            XmiEventKind::PitchBend {
                channel: 4,
                lsb: 1,
                msb: 2,
            },
        ),
        (
            1,
            XmiEventKind::NoteOff {
                channel: 4,
                key: 60,
                velocity: 8,
            },
        ),
        (1, cc(4, 108, 2)),
        (1, cc(4, 120, 3)),
        (1, cc(4, 121, 0)),
    ];
    let sequence = seq(&values, 2);
    let mut s = XmiScheduler::new(sequence.clone(), SampleClock::new(1, 120).unwrap()).unwrap();
    let events = drain(&mut s, 2);
    assert_eq!(
        events
            .iter()
            .filter_map(
                |event| if let ScheduledKind::Source { event_index, .. } = event.kind {
                    Some(event_index)
                } else {
                    None
                }
            )
            .collect::<Vec<_>>(),
        (0..=values.len()).collect::<Vec<_>>()
    );
    assert!(events.iter().all(|event| event.frame == 0));
    assert!(events[0].midi_message().is_none());
    assert!(matches!(
        sequence.events[0].kind,
        XmiEventKind::Meta { kind: 0x51, .. }
    ));
    assert_eq!(
        midis(&events),
        [
            (0, 0xc4, 5, 0),
            (0, 0xd4, 6, 0),
            (0, 0xa4, 60, 7),
            (1, 0xe4, 1, 2),
            (1, 0x84, 60, 8),
            (1, 0xb4, 108, 2),
            (1, 0xb4, 120, 3),
            (1, 0xb4, 121, 0)
        ]
    );
}

#[test]
fn explicit_zero_delays_fail_preflight_but_zero_duration_still_schedules() {
    fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut bytes = tag.to_vec();
        bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        bytes.extend_from_slice(payload);
        if payload.len() % 2 == 1 {
            bytes.push(0);
        }
        bytes
    }
    fn parsed(events: &[u8]) -> Arc<XmiSequence> {
        let mut bytes = chunk(b"FORM", &[&b"XDIR"[..], &chunk(b"INFO", &[1, 0])].concat());
        let form = chunk(b"FORM", &[&b"XMID"[..], &chunk(b"EVNT", events)].concat());
        bytes.extend(chunk(b"CAT ", &[&b"XMID"[..], &form].concat()));
        Arc::new(
            openeq_assets::audio::xmi::XmiFile::parse(&bytes)
                .unwrap()
                .sequences
                .remove(0),
        )
    }
    let clock = SampleClock::miles_default(48000).unwrap();
    for (bytes, delay) in [
        (&[0, 0xc0, 1, 0xff, 0x2f, 0][..], 0),
        (&[1, 0, 0xc0, 1, 0xff, 0x2f, 0][..], 1),
    ] {
        let sequence = parsed(bytes);
        assert_eq!(sequence.events[0].delay_ticks, delay);
        assert!(sequence.events[0].has_zero_delay_byte);
        let report = preflight(&sequence, clock);
        assert!(!report.is_supported());
        assert_eq!(
            report.diagnostics,
            [ScheduleDiagnostic {
                first_event: Some(0),
                issue: ScheduleIssue::UnsupportedZeroDelay,
                occurrences: 1,
            }]
        );
        let error = XmiScheduler::new(sequence, clock).err().unwrap();
        assert_eq!(error.event_index, Some(0));
        assert_eq!(error.issue, ScheduleIssue::UnsupportedZeroDelay);
    }
    let sequence = parsed(&[0x90, 60, 90, 0, 2, 0xff, 0x2f, 0]);
    assert!(
        sequence
            .events
            .iter()
            .all(|event| !event.has_zero_delay_byte)
    );
    let mut scheduler = XmiScheduler::new(sequence, clock).unwrap();
    assert_eq!(
        midis(&drain(&mut scheduler, 512)),
        [(0, 0x90, 60, 90), (1, 0x80, 60, 0)]
    );
}

#[test]
fn unsupported_constructs_fail_before_any_output_and_report_occurrences() {
    let mut values = Vec::new();
    for controller in [106, 109, 110, 111, 115, 118, 119] {
        values.push((0, cc(0, controller, 0)));
    }
    values.push((0, cc(0, 109, 127)));
    values.push((
        0,
        XmiEventKind::SysEx {
            status: 0xf0,
            payload: vec![0x41],
        },
    ));
    let sequence = seq(&values, 1);
    let report = preflight(&sequence, SampleClock::miles_default(48000).unwrap());
    assert!(!report.is_supported());
    assert_eq!(report.diagnostics.len(), 8);
    assert_eq!(
        report
            .diagnostics
            .iter()
            .find(|d| d.issue == ScheduleIssue::UnsupportedController(109))
            .unwrap()
            .occurrences,
        2
    );
    let error = XmiScheduler::new(sequence, SampleClock::miles_default(48000).unwrap())
        .err()
        .unwrap();
    assert_eq!(error.event_index, Some(0));
    assert_eq!(error.issue, ScheduleIssue::UnsupportedController(106));
}

#[test]
fn capacity_work_and_sequence_guards_are_bounded() {
    let values = vec![(0, note(60, 10)); 32];
    let report = preflight(
        &seq(&values, 10),
        SampleClock::miles_default(48000).unwrap(),
    );
    assert!(report.is_supported());
    assert_eq!(report.peak_active_notes, 32);
    let mut full = values.clone();
    full.push((0, note(60, 1)));
    assert!(
        preflight(&seq(&full, 10), SampleClock::miles_default(48000).unwrap())
            .diagnostics
            .iter()
            .any(|d| d.issue == ScheduleIssue::ActiveNoteLimit)
    );
    let mut s = scheduler(&values, 10);
    assert!(s.next_batch(0).is_err());
    assert!(s.next_batch(MAX_BATCH_EVENTS + 1).is_err());
    assert_eq!(s.next_frame().unwrap(), Some(0));
    assert_eq!(s.next_batch(32).unwrap().len(), 32);
    assert_eq!(s.next_frame().unwrap(), Some(4000));
    assert_eq!(s.cancel().len(), 32);
    let clock = SampleClock::miles_default(48000).unwrap();
    for sequence in [
        seq(&[(2, note(60, 1)), (1, note(61, 1))], 3),
        seq(&[(2, note(60, 1))], 1),
        seq(
            &[(
                0,
                XmiEventKind::NoteOn {
                    channel: 16,
                    key: 128,
                    velocity: 90,
                    duration_ticks: 1,
                },
            )],
            1,
        ),
    ] {
        assert!(!preflight(&sequence, clock).is_supported());
    }
    let mut missing = (*seq(&[], 0)).clone();
    missing.events.clear();
    assert!(!preflight(&missing, clock).is_supported());
    let huge = seq(&vec![(0, cc(0, 1, 0)); MAX_XMI_SEQUENCE_EVENTS], 0);
    assert_eq!(
        preflight(&huge, clock).diagnostics[0].issue,
        ScheduleIssue::EventLimit
    );
    assert!(
        preflight(
            &seq(&[(u64::MAX, note(60, 0))], u64::MAX),
            SampleClock::new(1, 1).unwrap()
        )
        .diagnostics
        .iter()
        .any(|d| d.issue == ScheduleIssue::TickOverflow)
    );
}

#[test]
fn batch_boundaries_do_not_change_timing_or_note_identity() {
    let values = (0..1000)
        .map(|tick| (tick, note((tick % 128) as u8, 7)))
        .collect::<Vec<_>>();
    let expected = drain(&mut scheduler(&values, 1010), 512);
    for chunk in [1, 7, 32, 511] {
        assert_eq!(drain(&mut scheduler(&values, 1010), chunk), expected);
    }
}

#[test]
#[ignore = "requires the original XMI files in EQ_DIR; silent parser/scheduler audit"]
fn original_xmi_scheduler_coverage() {
    use std::collections::{BTreeMap, BTreeSet};
    let base = std::env::var_os("EQ_DIR").expect("set EQ_DIR");
    let mut paths = std::fs::read_dir(base)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("xmi"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    assert_eq!(paths.len(), 79);
    let mut counts = BTreeMap::<String, usize>::new();
    let (mut total, mut supported, mut linear, mut peak, mut output_count) = (0, 0, 0, 0, 0);
    for path in paths {
        let file =
            openeq_assets::audio::xmi::XmiFile::parse(&std::fs::read(&path).unwrap()).unwrap();
        for sequence in file.sequences {
            total += 1;
            let report = preflight(&sequence, SampleClock::miles_default(48000).unwrap());
            peak = peak.max(report.peak_active_notes);
            if !report.is_supported() {
                let mut reasons = BTreeSet::new();
                for diagnostic in &report.diagnostics {
                    reasons.insert(format!("{:?}", diagnostic.issue));
                }
                for reason in &reasons {
                    *counts.entry(reason.clone()).or_default() += 1;
                }
                println!(
                    "unsupported {} ordinal{}: {}",
                    path.file_name().unwrap().to_string_lossy(),
                    sequence.ordinal.0,
                    reasons.into_iter().collect::<Vec<_>>().join(", ")
                );
                continue;
            }
            supported += 1;
            if report.end_frame.is_none() {
                // Original loops have dedicated bounded/native trace tests;
                // two intentionally never reach EOT.
                continue;
            }
            linear += 1;
            let mut scheduler = XmiScheduler::new(
                Arc::new(sequence),
                SampleClock::miles_default(48000).unwrap(),
            )
            .unwrap();
            let mut previous_frame = 0;
            while !scheduler.is_finished() {
                let events = scheduler.next_batch(512).unwrap();
                assert!(events.len() <= 512);
                for event in events {
                    assert!(event.frame >= previous_frame);
                    previous_frame = event.frame;
                    output_count += 1;
                }
            }
            assert!(scheduler.cancel().is_empty());
        }
    }
    assert_eq!(total, 389);
    println!(
        "XMI scheduler coverage: {supported}/{total} sequences, peak{peak}/32 active notes, {output_count} scheduled outputs; reasons {counts:?}"
    );
    assert_eq!(
        (supported, linear, peak, output_count),
        (388, 384, 31, 927_156)
    );
    assert_eq!(counts, BTreeMap::from([("UnsupportedSysEx".into(), 1)]));
}
