use super::tests::{cc, drain, note, seq};
use super::*;

fn sysex(status: u8, payload: &[u8]) -> XmiEventKind {
    XmiEventKind::SysEx {
        status,
        payload: payload.to_vec(),
    }
}

#[test]
fn sysex_preflight_rejects_continuations_malformed_and_oversized_packets() {
    let clock = SampleClock::miles_default(48000).unwrap();
    for (status, payload) in [
        (0xf7, vec![0x41, 0xf7]),
        (0xf0, vec![]),
        (0xf0, vec![0x41]),
        (0xf0, vec![0x80, 0xf7]),
        (0xf0, vec![0xf7, 0xf7]),
        (0xf0, [vec![0; 1535], vec![0xf7]].concat()),
    ] {
        let source = seq(&[(0, note(60, 1)), (1, sysex(status, &payload))], 2);
        let error = XmiScheduler::new(source, clock).err().unwrap();
        assert_eq!(error.event_index, Some(1));
        assert_eq!(error.issue, ScheduleIssue::UnsupportedSysEx);
    }
    let mut payload = vec![0; 1535];
    *payload.last_mut().unwrap() = 0xf7;
    assert!(XmiScheduler::new(seq(&[(0, sysex(0xf0, &payload))], 0), clock).is_ok());
}

#[test]
fn packet_order_survives_releases_loop_reentry_and_small_batches() {
    let source = seq(
        &[
            (0, note(60, 1)),
            (1, sysex(0xf0, &[0x7d, 1, 0xf7])),
            (1, cc(0, 116, 2)),
            (1, sysex(0xf0, &[0x7d, 2, 0xf7])),
            (2, cc(0, 117, 127)),
        ],
        3,
    );
    let clock = SampleClock::miles_default(48000).unwrap();
    let mut expected = None;
    for batch in [1, 7, 512] {
        let mut schedule = XmiScheduler::new(Arc::clone(&source), clock).unwrap();
        let events = drain(&mut schedule, batch);
        if let Some(expected) = &expected {
            assert_eq!(&events, expected);
        } else {
            expected = Some(events.clone());
        }
        let packets: Vec<_> = events
            .iter()
            .filter_map(|event| {
                schedule
                    .sysex_payload(*event)
                    .map(|payload| (event.tick, payload.to_vec()))
            })
            .collect();
        assert_eq!(
            packets,
            [
                (1, vec![0x7d, 1, 0xf7]),
                (1, vec![0x7d, 2, 0xf7]),
                (2, vec![0x7d, 2, 0xf7]),
                (3, vec![0x7d, 2, 0xf7]),
                (4, vec![0x7d, 2, 0xf7]),
                (5, vec![0x7d, 2, 0xf7])
            ]
        );
        assert!(matches!(events[1].kind, ScheduledKind::Release { .. }));
        assert!(matches!(
            events[2].kind,
            ScheduledKind::SysEx { event_index: 1 }
        ));
        assert!(events.iter().all(|event| event.frame == event.tick * 400));
        assert_eq!(schedule.sysex_payload(events[0]), None);
    }
    let mut schedule = XmiScheduler::new(source, clock).unwrap();
    schedule.next_event().unwrap();
    assert_eq!(schedule.cancel().len(), 1);
    assert_eq!(schedule.next_event().unwrap(), None);
}

fn digest(hash: &mut u64, bytes: impl IntoIterator<Item = u8>) {
    for byte in bytes {
        *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
}

#[test]
#[ignore = "requires original The Deep XMI; compares isolated native byte/tick digests without playback"]
fn original_thedeep_matches_native_channel_and_sysex_trace() {
    let base = std::path::PathBuf::from(std::env::var_os("EQ_DIR").expect("set EQ_DIR"));
    let file = openeq_assets::audio::xmi::XmiFile::parse(
        &std::fs::read(base.join("thedeep.xmi")).unwrap(),
    )
    .unwrap();
    let source = Arc::new(file.sequences[0].clone());
    let immutable = (*source).clone();
    for batch in [1, 7, 512] {
        let mut schedule =
            XmiScheduler::new(Arc::clone(&source), SampleClock::new(1, 1).unwrap()).unwrap();
        let mut ordinary = 0xcbf29ce484222325;
        let mut sysex = ordinary;
        let mut combined = ordinary;
        let mut midi_count = 0;
        let mut packet_count = 0;
        let mut source_count = 0;
        let events = drain(&mut schedule, batch);
        for event in &events {
            if let ScheduledKind::Source { event_index, .. } = event.kind
                && !matches!(source.events[event_index].kind, XmiEventKind::Meta { .. })
            {
                source_count += 1;
            }
            if let Some(message) = event.midi_message() {
                let bytes: Vec<_> = event
                    .tick
                    .to_le_bytes()
                    .into_iter()
                    .chain([message.status, message.data1, message.data2])
                    .collect();
                digest(&mut ordinary, bytes.iter().copied());
                digest(&mut combined, std::iter::once(0).chain(bytes));
                midi_count += 1;
            } else if let ScheduledKind::SysEx { event_index } = event.kind {
                let payload = schedule.sysex_payload(*event).unwrap();
                let offset = u64::from(source.events[event_index].status_offset);
                let bytes: Vec<_> = event
                    .tick
                    .to_le_bytes()
                    .into_iter()
                    .chain(offset.to_le_bytes())
                    .chain((payload.len() as u64 + 1).to_le_bytes())
                    .chain(std::iter::once(0xf0))
                    .chain(payload.iter().copied())
                    .collect();
                digest(&mut sysex, bytes.iter().copied());
                digest(&mut combined, std::iter::once(1).chain(bytes));
                packet_count += 1;
            }
        }
        assert_eq!((midi_count, packet_count, source_count), (2443, 22, 1349));
        assert_eq!(ordinary, 0x7c7b681d2ef68c11);
        assert_eq!(sysex, 0xff375c25ef67e49e);
        assert_eq!(combined, 0x698546ba61a0c3e2);
        assert_eq!(events.last().unwrap().tick, 23925);
        assert!(schedule.cancel().is_empty());
        assert_eq!(*source, immutable);
    }
}
