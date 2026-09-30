use super::*;

fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut bytes = tag.to_vec();
    bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    bytes.extend_from_slice(payload);
    if payload.len() % 2 == 1 {
        bytes.push(0);
    }
    bytes
}
fn container(tag: &[u8; 4], kind: &[u8; 4], children: &[Vec<u8>]) -> Vec<u8> {
    let mut payload = kind.to_vec();
    payload.extend(children.iter().flatten());
    chunk(tag, &payload)
}
fn file(sequences: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = container(
        b"FORM",
        b"XDIR",
        &[chunk(b"INFO", &(sequences.len() as u16).to_le_bytes())],
    );
    bytes.extend(container(b"CAT ", b"XMID", sequences));
    bytes
}
fn sequence(children: &[Vec<u8>]) -> Vec<u8> {
    container(b"FORM", b"XMID", children)
}
fn event_file(events: &[u8]) -> Vec<u8> {
    file(&[sequence(&[chunk(b"EVNT", events)])])
}
fn parse_events(events: &[u8]) -> XmiSequence {
    XmiFile::parse(&event_file(events))
        .unwrap()
        .sequences
        .remove(0)
}
fn reject_events(events: &[u8], message: &str) {
    let error = XmiFile::parse(&event_file(events)).unwrap_err();
    assert_eq!(error.sequence, Some(XmiSequenceOrdinal(0)));
    assert!(error.message.contains(message), "{error}");
}

#[test]
fn explicit_zero_delay_provenance_preserves_additive_ticks() {
    for (bytes, delay) in [
        (&[0, 0xc0, 1, 0xff, 0x2f, 0][..], 0),
        (&[1, 0, 0xc0, 1, 0xff, 0x2f, 0][..], 1),
    ] {
        let sequence = parse_events(bytes);
        assert!(sequence.events[0].has_zero_delay_byte);
        assert_eq!(sequence.events[0].delay_ticks, delay);
        assert_eq!(sequence.events[0].tick, delay);
        assert_eq!(sequence.end_tick, delay);
        assert!(!sequence.events[1].has_zero_delay_byte);
    }
    // Payload zeros, zero duration, and internal EOT padding are not delays.
    let sequence = parse_events(&[0x90, 60, 0, 0, 1, 0xff, 0x2f, 0, 0]);
    assert!(sequence.has_eot_padding);
    assert!(
        sequence
            .events
            .iter()
            .all(|event| !event.has_zero_delay_byte)
    );
    assert_eq!(sequence.events[0].note_end_tick(), Some(0));
}

#[test]
fn additive_delays_durations_overlap_and_same_tick_order_remain_independent() {
    let sequence = parse_events(&[
        0x7f, 0x01, 0x91, 60, 100, 0x81, 0x00, // tick128, duration128
        0x91, 60, 0, 0, // same-tick/source order, duration0, velocity0 stays note-on
        2, 0x91, 60, 80, 1, // overlapping same key, duration1 at130
        0xb1, 64, 127, // sustain remains raw at130
        0x7e, 0xff, 0x2f, 0, // EOT256
    ]);
    assert_eq!(sequence.events.len(), 5);
    assert_eq!(sequence.end_tick, 256);
    assert_eq!(
        sequence
            .events
            .iter()
            .map(|e| (e.offset, e.status_offset, e.delay_ticks, e.tick))
            .collect::<Vec<_>>(),
        [
            (0, 2, 128, 128),
            (7, 7, 0, 128),
            (11, 12, 2, 130),
            (16, 16, 0, 130),
            (19, 20, 126, 256)
        ]
    );
    assert_eq!(sequence.events[0].note_end_tick(), Some(256));
    assert_eq!(sequence.events[1].note_end_tick(), Some(128));
    assert_eq!(sequence.events[2].note_end_tick(), Some(131));
    assert_eq!(sequence.events[3].note_end_tick(), None);
    assert!(matches!(
        sequence.events[1].kind,
        XmiEventKind::NoteOn {
            channel: 1,
            key: 60,
            velocity: 0,
            duration_ticks: 0
        }
    ));
    // A later scheduler owns EOT cleanup; the parser does not truncate duration.
    let late = parse_events(&[0x90, 60, 90, 100, 0xff, 0x2f, 0]);
    assert_eq!(late.end_tick, 0);
    assert_eq!(late.events[0].note_end_tick(), Some(100));
}

#[test]
fn channel_shapes_meta_and_sysex_payloads_preserve_opaque_values() {
    let sequence = parse_events(&[
        0x8f, 127, 126, 0xaf, 125, 124, 0xbf, 120, 3, 0xcf, 123, 0xdf, 122, 0xef, 121, 120, 0xff,
        0x51, 3, 7, 0xa1, 0x20, // opaque tempo
        0xff, 0x7f, 4, b'F', b'O', b'R', b'M', // no magic scan
        0xf0, 3, 0x41, 0xff, 0xf7, 0xf7, 2, 0, 0xff, 1, 0xff, 0x2f, 0,
    ]);
    assert_eq!(
        sequence
            .events
            .iter()
            .map(|e| e.kind.clone())
            .collect::<Vec<_>>(),
        [
            XmiEventKind::NoteOff {
                channel: 15,
                key: 127,
                velocity: 126
            },
            XmiEventKind::PolyPressure {
                channel: 15,
                key: 125,
                pressure: 124
            },
            XmiEventKind::Controller {
                channel: 15,
                controller: 120,
                value: 3
            },
            XmiEventKind::ProgramChange {
                channel: 15,
                program: 123
            },
            XmiEventKind::ChannelPressure {
                channel: 15,
                pressure: 122
            },
            XmiEventKind::PitchBend {
                channel: 15,
                lsb: 121,
                msb: 120
            },
            XmiEventKind::Meta {
                kind: 0x51,
                payload: vec![7, 0xa1, 0x20]
            },
            XmiEventKind::Meta {
                kind: 0x7f,
                payload: b"FORM".to_vec()
            },
            XmiEventKind::SysEx {
                status: 0xf0,
                payload: vec![0x41, 0xff, 0xf7]
            },
            XmiEventKind::SysEx {
                status: 0xf7,
                payload: vec![0, 0xff]
            },
            XmiEventKind::Meta {
                kind: 0x2f,
                payload: vec![]
            },
        ]
    );
    assert_eq!(sequence.end_tick, 1); // tempo payload never changes tick scale
    assert_eq!(sequence.events[10].tick, 1);
}

#[test]
fn tables_keep_raw_widths_order_and_branch_delays() {
    let timbres = chunk(b"TIMB", &[2, 0, 0xff, 0x80, 48, 127]);
    let mut records = vec![3, 0];
    for (id, offset) in [(u16::MAX, 0u32), (0, 2), (0, 6)] {
        records.extend_from_slice(&id.to_le_bytes());
        records.extend_from_slice(&offset.to_le_bytes());
    }
    let events = [0xc0, 3, 8, 0xe1, 127, 127, 0xff, 0x2f, 0];
    let bytes = file(&[
        sequence(&[chunk(b"EVNT", &events), timbres, chunk(b"RBRN", &records)]),
        sequence(&[
            chunk(b"TIMB", &[0, 0]),
            chunk(b"RBRN", &[0, 0]),
            chunk(b"EVNT", &[0xff, 0x2f, 0]),
        ]),
    ]);
    let parsed = XmiFile::parse(&bytes).unwrap();
    let first = &parsed.sequences[0];
    assert_eq!(first.ordinal.index(), 0);
    assert_eq!(parsed.sequences[1].ordinal.index(), 1);
    assert_eq!(
        first.timbres.as_deref(),
        Some([[255, 128], [48, 127]].as_slice())
    );
    assert_eq!(
        first.branches.as_deref(),
        Some(
            [
                XmiBranch {
                    marker_id: u16::MAX,
                    event_offset: 0,
                    event_index: 0
                },
                XmiBranch {
                    marker_id: 0,
                    event_offset: 2,
                    event_index: 1
                },
                XmiBranch {
                    marker_id: 0,
                    event_offset: 6,
                    event_index: 2
                },
            ]
            .as_slice()
        )
    );
    assert_eq!(
        &bytes[first.evnt_payload_offset
            ..first.evnt_payload_offset + first.evnt_payload_len as usize],
        &events
    );
    assert_eq!(first.events[1].offset, 2);
    assert_eq!(first.events[1].status_offset, 3);
    assert_eq!(first.events[1].delay_ticks, 8);
    assert_eq!(parsed.sequences[1].timbres, Some(vec![]));
    assert_eq!(parsed.sequences[1].branches, Some(vec![]));
    assert_eq!(parse_events(&[0xff, 0x2f, 0]).timbres, None);
}

#[test]
fn branch_targets_must_include_delay_and_cannot_enter_status_data_or_padding() {
    for target in [1u32, 2, 3, 5, 6, 7, 8, 9, u32::MAX] {
        // Valid event starts:0 (delay8/bend) and4 (two zero delays/EOT).
        let events = [8, 0xe1, 127, 127, 0, 0, 0xff, 0x2f, 0, 0];
        let mut branch = vec![1, 0, 0, 0];
        branch.extend_from_slice(&target.to_le_bytes());
        let bytes = file(&[sequence(&[
            chunk(b"RBRN", &branch),
            chunk(b"EVNT", &events),
        ])]);
        let error = XmiFile::parse(&bytes).unwrap_err();
        assert_eq!(error.message, "branch target is not an event-start offset");
        assert_eq!(error.sequence, Some(XmiSequenceOrdinal(0)));
    }
}

#[test]
fn eot_is_required_and_only_one_internal_zero_is_accepted() {
    let plain = parse_events(&[0xff, 0x2f, 0]); // odd payload requires external IFF pad
    assert!(!plain.has_eot_padding);
    let padded = parse_events(&[0xff, 0x2f, 0, 0]);
    assert!(padded.has_eot_padding);
    for events in [&[][..], &[0xc0, 1][..]] {
        reject_events(events, "missing end-of-track");
    }
    reject_events(&[0xc0, 1, 0], "truncated event");
    reject_events(&[0xff, 0x2f, 1, 0], "end-of-track payload");
    for tail in [&[0, 0][..], &[1][..], &[0xff, 0x2f, 0][..]] {
        let mut events = vec![0xff, 0x2f, 0];
        events.extend_from_slice(tail);
        reject_events(&events, "data after end-of-track");
    }
}

#[test]
fn vlqs_data_bytes_and_payloads_are_checked_before_allocation() {
    let sequence = parse_events(&[0x90, 60, 90, 0xff, 0xff, 0xff, 0x7f, 0xff, 0x2f, 0]);
    assert_eq!(sequence.events[0].note_end_tick(), Some(0x0fff_ffff));
    for events in [
        &[0x90, 60, 90, 0x81][..],
        &[0xff, 1, 0x81][..],
        &[0xf0, 0x81][..],
    ] {
        reject_events(events, "truncated event");
    }
    for prefix in [&[0x90, 60, 90][..], &[0xff, 1][..], &[0xf7][..]] {
        let mut events = prefix.to_vec();
        events.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0]);
        reject_events(&events, "VLQ exceeds four");
    }
    reject_events(&[0xff, 1, 0xff, 0xff, 0xff, 0x7f], "payload exceeds EVNT");
    reject_events(&[0xf7, 10, 1], "payload exceeds EVNT");
    for status in [0x80, 0x90, 0xa0, 0xb0, 0xc0, 0xd0, 0xe0] {
        reject_events(&[status, 128], "data byte has status bit");
    }
    for status in [
        0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf8, 0xf9, 0xfa, 0xfb, 0xfc, 0xfd, 0xfe,
    ] {
        reject_events(&[status], "unsupported system status");
    }
    assert_eq!(
        checked_ticks(u64::MAX - 1, 1, 7, XmiSequenceOrdinal(3)).unwrap(),
        u64::MAX
    );
    let overflow = checked_ticks(u64::MAX, 1, 7, XmiSequenceOrdinal(3)).unwrap_err();
    assert_eq!(overflow.offset, 7);
    assert_eq!(overflow.sequence, Some(XmiSequenceOrdinal(3)));
}

#[test]
fn unknown_leaf_chunks_keep_context_and_ignore_magic_inside_payload() {
    let opaque = b"FORM\0\0\0\xffEVNT\0";
    let mut bytes = chunk(b"JUNK", opaque);
    bytes.extend(container(
        b"FORM",
        b"XDIR",
        &[chunk(b"ODD!", &[9]), chunk(b"INFO", &1u16.to_le_bytes())],
    ));
    bytes.extend(container(
        b"CAT ",
        b"XMID",
        &[
            chunk(b"MORE", &[]),
            sequence(&[
                chunk(b"RAWW", &[255, 128, 1]),
                chunk(b"EVNT", &[0xff, 0x2f, 0]),
            ]),
        ],
    ));
    let parsed = XmiFile::parse(&bytes).unwrap();
    assert_eq!(
        parsed
            .opaque_chunks
            .iter()
            .map(|c| c.scope)
            .collect::<Vec<_>>(),
        [
            XmiChunkScope::File,
            XmiChunkScope::Directory,
            XmiChunkScope::Catalog,
            XmiChunkScope::Sequence(XmiSequenceOrdinal(0))
        ]
    );
    assert_eq!(parsed.opaque_chunks[0].payload, opaque);
    assert_eq!(parsed.opaque_chunks[1].payload, [9]);
    for chunk in &parsed.opaque_chunks {
        assert_eq!(
            &bytes[chunk.header_offset..chunk.header_offset + 4],
            &chunk.tag
        );
    }
    // The external odd-byte alignment value is opaque, not required to be zero.
    bytes[8 + opaque.len()] = 0xe3;
    assert!(XmiFile::parse(&bytes).is_ok());
}

#[test]
fn required_chunks_counts_and_nesting_cannot_be_duplicated_or_hidden() {
    let eot = chunk(b"EVNT", &[0xff, 0x2f, 0]);
    for tag in [b"EVNT", b"TIMB", b"RBRN"] {
        let value = if tag == b"EVNT" {
            eot.clone()
        } else {
            chunk(tag, &[0, 0])
        };
        let bytes = file(&[sequence(&[value.clone(), value, eot.clone()])]);
        assert!(
            XmiFile::parse(&bytes)
                .unwrap_err()
                .message
                .starts_with("duplicate")
        );
    }
    assert_eq!(
        XmiFile::parse(&file(&[sequence(&[])])).unwrap_err().message,
        "missing EVNT"
    );
    let mut bytes = event_file(&[0xff, 0x2f, 0]);
    bytes[20..22].copy_from_slice(&2u16.to_le_bytes());
    assert_eq!(
        XmiFile::parse(&bytes).unwrap_err().message,
        "INFO sequence count mismatch"
    );
    for count in [0u16, 257, u16::MAX] {
        bytes[20..22].copy_from_slice(&count.to_le_bytes());
        assert!(
            XmiFile::parse(&bytes)
                .unwrap_err()
                .message
                .contains("sequence count limit")
        );
    }
    for children in [
        vec![],
        vec![chunk(b"INFO", &[1])],
        vec![chunk(b"INFO", &[1, 0]), chunk(b"INFO", &[1, 0])],
    ] {
        assert!(XmiFile::parse(&container(b"FORM", b"XDIR", &children)).is_err());
    }
    let nested = file(&[sequence(&[
        container(b"FORM", b"XMID", std::slice::from_ref(&eot)),
        eot.clone(),
    ])]);
    assert_eq!(
        XmiFile::parse(&nested).unwrap_err().message,
        "chunk is invalid at this nesting level"
    );
    for invalid in [
        eot,
        container(b"CAT ", b"XMID", &[]),
        container(b"FORM", b"XMID", &[]),
        container(b"LIST", b"XMID", &[]),
    ] {
        assert!(XmiFile::parse(&invalid).is_err());
    }
    let mut duplicate = event_file(&[0xff, 0x2f, 0]);
    duplicate.extend(event_file(&[0xff, 0x2f, 0]));
    assert!(XmiFile::parse(&duplicate).is_err());
}

#[test]
fn every_file_prefix_parent_extent_and_table_length_is_checked() {
    let bytes = event_file(&[0xc0, 1, 0xff, 0x2f, 0]);
    for end in 0..bytes.len() {
        assert!(XmiFile::parse(&bytes[..end]).is_err(), "prefix{end}");
    }
    for index in [4, 26, 38, 50] {
        // directory, catalog, sequence, EVNT BE length
        let mut broken = bytes.clone();
        broken[index..index + 4].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(XmiFile::parse(&broken).is_err(), "length offset{index}");
    }
    for tag in [b"TIMB", b"RBRN"] {
        for payload in [
            &[][..],
            &[1][..],
            &[1, 0][..],
            &[0, 0, 1][..],
            &[0xff, 0xff][..],
        ] {
            let broken = file(&[sequence(&[
                chunk(tag, payload),
                chunk(b"EVNT", &[0xff, 0x2f, 0]),
            ])]);
            assert!(XmiFile::parse(&broken).is_err());
        }
    }
    // An odd child payload requires its alignment byte within its parent.
    let mut odd = chunk(b"ODD!", &[1]);
    odd.pop();
    let broken = container(b"FORM", b"XDIR", &[chunk(b"INFO", &[1, 0]), odd]);
    assert!(
        XmiFile::parse(&broken)
            .unwrap_err()
            .message
            .contains("exceeds parent")
    );
}

#[test]
fn all_resource_limits_bound_work_and_allocations() {
    assert!(
        XmiFile::parse(&vec![0; MAX_XMI_BYTES + 1])
            .unwrap_err()
            .message
            .contains("source byte limit")
    );
    let eot = sequence(&[chunk(b"EVNT", &[0xff, 0x2f, 0])]);
    let many = file(&vec![eot.clone(); MAX_XMI_SEQUENCES]);
    assert_eq!(
        XmiFile::parse(&many).unwrap().sequences.len(),
        MAX_XMI_SEQUENCES
    );
    let mut too_many = file(&vec![eot; MAX_XMI_SEQUENCES + 1]);
    too_many[20..22].copy_from_slice(&(MAX_XMI_SEQUENCES as u16).to_le_bytes());
    assert!(
        XmiFile::parse(&too_many)
            .unwrap_err()
            .message
            .contains("sequence count limit")
    );
    let mut empty_chunks = Vec::new();
    for _ in 0..=MAX_XMI_CHUNKS {
        empty_chunks.extend(chunk(b"NONE", &[]));
    }
    assert!(
        XmiFile::parse(&empty_chunks)
            .unwrap_err()
            .message
            .contains("chunk count limit")
    );
    let mut events = [0xc0, 0].repeat(MAX_XMI_SEQUENCE_EVENTS - 1);
    events.extend([0xff, 0x2f, 0]);
    assert_eq!(
        XmiFile::parse(&event_file(&events)).unwrap().sequences[0]
            .events
            .len(),
        MAX_XMI_SEQUENCE_EVENTS
    );
    events.splice(0..0, [0xc0, 0]);
    reject_events(&events, "event count limit");
    let mut sixty_thousand = [0xc0, 0].repeat(59_999);
    sixty_thousand.extend([0xff, 0x2f, 0]);
    let five = file(&vec![sequence(&[chunk(b"EVNT", &sixty_thousand)]); 5]);
    let error = XmiFile::parse(&five).unwrap_err();
    assert_eq!(error.sequence, Some(XmiSequenceOrdinal(4)));
    assert_eq!(error.message, "event count limit exceeded");
    for (tag, stride) in [(b"TIMB", 2), (b"RBRN", 6)] {
        let mut table = (MAX_XMI_TABLE_RECORDS as u16).to_le_bytes().to_vec();
        table.resize(2 + MAX_XMI_TABLE_RECORDS * stride, 0);
        assert!(
            XmiFile::parse(&file(&[sequence(&[
                chunk(tag, &table),
                chunk(b"EVNT", &[0xff, 0x2f, 0])
            ])]))
            .is_ok()
        );
        table[..2].copy_from_slice(&((MAX_XMI_TABLE_RECORDS + 1) as u16).to_le_bytes());
        table.resize(table.len() + stride, 0);
        assert!(
            XmiFile::parse(&file(&[sequence(&[
                chunk(tag, &table),
                chunk(b"EVNT", &[0xff, 0x2f, 0])
            ])]))
            .unwrap_err()
            .message
            .contains("table record limit")
        );
    }
}

#[test]
#[ignore = "requires the original 79 XMI files in EQ_DIR; parser only, no audio"]
fn original_xmi_container_and_sequence_sweep() {
    let base =
        std::env::var_os("EQ_DIR").expect("set EQ_DIR to the original EverQuest installation");
    let mut paths = std::fs::read_dir(base)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("xmi"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    assert_eq!(
        paths.len(),
        79,
        "this fixture expects the researched installation"
    );
    let (mut sequences, mut events, mut timb, mut timbres, mut rbrn, mut branches, mut padding) =
        (0, 0, 0, 0, 0, 0, 0);
    let (
        mut notes,
        mut bends,
        mut controls,
        mut programs,
        mut opaque,
        mut zero_notes,
        mut tempos,
        mut sysex,
    ) = (0, 0, 0, 0, 0, 0, 0, 0);
    let (mut max_tick, mut max_duration, mut max_events, mut max_evnt) = (0, 0, 0, 0);
    for path in paths {
        let bytes = std::fs::read(&path).unwrap();
        let file =
            XmiFile::parse(&bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(file.opaque_chunks.is_empty(), "{}", path.display());
        let name = path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_ascii_lowercase();
        if name == "gfaydark.xmi" {
            assert_eq!(
                file.sequences
                    .iter()
                    .map(|s| s.end_tick)
                    .collect::<Vec<_>>(),
                [1767, 2351, 2351, 1767, 15188, 4596]
            );
        }
        if name == "griegsend.xmi" {
            assert_eq!(file.sequences[0].end_tick, 39431);
        }
        if name == "gl.xmi" {
            assert_eq!(file.sequences.len(), 24);
        }
        if name == "qeynos.xmi" {
            assert_eq!(file.sequences.len(), 13);
        }
        if name == "befallen.xmi" {
            let seq = &file.sequences[11];
            let branch = seq
                .branches
                .as_ref()
                .unwrap()
                .iter()
                .find(|branch| branch.marker_id == 0)
                .unwrap();
            assert_eq!(branch.event_offset, 765);
            let target = &seq.events[branch.event_index];
            assert_eq!(target.delay_ticks, 8);
            assert_eq!(target.status_offset, 766);
            assert!(matches!(
                target.kind,
                XmiEventKind::PitchBend {
                    channel: 1,
                    lsb: 127,
                    msb: 127
                }
            ));
        }
        sequences += file.sequences.len();
        for sequence in file.sequences {
            events += sequence.events.len();
            max_tick = max_tick.max(sequence.end_tick);
            max_events = max_events.max(sequence.events.len());
            max_evnt = max_evnt.max(sequence.evnt_payload_len);
            padding += usize::from(sequence.has_eot_padding);
            if let Some(table) = sequence.timbres {
                timb += 1;
                timbres += table.len();
            }
            if let Some(table) = sequence.branches {
                rbrn += 1;
                branches += table.len();
            }
            for event in sequence.events {
                assert!(
                    !event.has_zero_delay_byte,
                    "explicit zero delay in {name} ordinal {} at {}",
                    sequence.ordinal.0, event.offset
                );
                if let Some(end) = event.note_end_tick() {
                    assert!(end <= sequence.end_tick);
                }
                match event.kind {
                    XmiEventKind::NoteOn { duration_ticks, .. } => {
                        notes += 1;
                        zero_notes += usize::from(duration_ticks == 0);
                        max_duration = max_duration.max(duration_ticks);
                    }
                    XmiEventKind::PitchBend { .. } => bends += 1,
                    XmiEventKind::Controller { .. } => controls += 1,
                    XmiEventKind::ProgramChange { .. } => programs += 1,
                    XmiEventKind::Meta { kind, .. } => {
                        opaque += 1;
                        tempos += usize::from(kind == 0x51);
                    }
                    XmiEventKind::SysEx { .. } => {
                        opaque += 1;
                        sysex += 1;
                    }
                    _ => panic!("unexpected researched channel shape in {name}"),
                }
            }
        }
    }
    assert_eq!(
        (sequences, events, timb, timbres, rbrn, branches, padding),
        (389, 548617, 387, 4392, 51, 58, 205)
    );
    assert_eq!(
        (
            notes, bends, controls, programs, opaque, zero_notes, tempos, sysex
        ),
        (392655, 82102, 60318, 4546, 8996, 83, 1772, 22)
    );
    assert_eq!(
        (max_tick, max_duration, max_events, max_evnt),
        (39431, 6442, 6896, 29918)
    );
}
