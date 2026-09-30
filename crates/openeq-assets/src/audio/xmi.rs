//! Bounded, silent XMI container/event decoding. No sequencing or synthesis.
//!
//! Native nonnegative EFF/EMT music selectors map directly to zero-based
//! container ordinals, not MIDI tracks or instrument IDs. Absolute ticks
//! preserve XMI's additive delays; tempo metadata does not rewrite time.
//! See `docs/XMI_PLAN.md` for the inspected format and unresolved control flow.

pub const MAX_XMI_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_XMI_SEQUENCES: usize = 256;
pub const MAX_XMI_SEQUENCE_EVENTS: usize = 65_536;
pub const MAX_XMI_FILE_EVENTS: usize = 262_144;
pub const MAX_XMI_TABLE_RECORDS: usize = 4096;
pub const MAX_XMI_CHUNKS: usize = 16_384;
/// Observed Miles default, not proof that a caller never changes its clock.
pub const DEFAULT_TICKS_PER_SECOND: u32 = 120;

/// Zero-based FORM XMID position. Native nonnegative EFF/EMT music selectors
/// map directly to this ordinal after resolving the selected file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct XmiSequenceOrdinal(pub u16);
impl XmiSequenceOrdinal {
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("XMI at byte {offset}, sequence {sequence:?}: {message}")]
pub struct XmiError {
    /// Absolute byte offset in the input file.
    pub offset: usize,
    pub sequence: Option<XmiSequenceOrdinal>,
    pub message: &'static str,
}
type Result<T> = std::result::Result<T, XmiError>;
fn error(offset: usize, sequence: Option<XmiSequenceOrdinal>, message: &'static str) -> XmiError {
    XmiError {
        offset,
        sequence,
        message,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XmiChunkScope {
    File,
    Directory,
    Catalog,
    Sequence(XmiSequenceOrdinal),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmiOpaqueChunk {
    pub scope: XmiChunkScope,
    pub tag: [u8; 4],
    pub header_offset: usize,
    /// Exact payload, excluding header and external IFF alignment byte.
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmiFile {
    /// Source-ordered FORM XMID records, with no selector remapping.
    pub sequences: Vec<XmiSequence>,
    /// Unknown leaf chunks in traversal/source order; no magic-string scanning.
    pub opaque_chunks: Vec<XmiOpaqueChunk>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmiSequence {
    pub ordinal: XmiSequenceOrdinal,
    pub form_offset: usize,
    pub evnt_payload_offset: usize,
    pub evnt_payload_len: u32,
    /// Raw pairs. Instrument/bank/percussion interpretation is not inferred.
    pub timbres: Option<Vec<[u8; 2]>>,
    pub branches: Option<Vec<XmiBranch>>,
    /// Source order is significant, including events with equal ticks.
    pub events: Vec<XmiEvent>,
    pub end_tick: u64,
    /// One optional zero after EOT *inside* EVNT, distinct from IFF alignment.
    pub has_eot_padding: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmiBranch {
    pub marker_id: u16,
    /// Original EVNT-relative offset, including preceding delay bytes.
    pub event_offset: u32,
    /// Validated index in `XmiSequence::events`, not an executed jump.
    pub event_index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmiEvent {
    /// EVNT-relative first delay byte, or status byte when there is no delay.
    pub offset: u32,
    pub status_offset: u32,
    pub delay_ticks: u64,
    /// A literal zero occurred among the additive delay bytes. Its native
    /// timing is unverified; this is distinct from a zero note duration.
    pub has_zero_delay_byte: bool,
    pub tick: u64,
    pub kind: XmiEventKind,
}
impl XmiEvent {
    /// A duration never advances the next source event's tick. This method does
    /// not generate a release or choose overlap/sustain/branching semantics.
    /// It returns the authored tick+duration; native scheduling imposes a
    /// one-tick minimum for zero durations and may stop notes earlier at EOT.
    pub fn note_end_tick(&self) -> Option<u64> {
        if let XmiEventKind::NoteOn { duration_ticks, .. } = self.kind {
            self.tick.checked_add(u64::from(duration_ticks))
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XmiEventKind {
    NoteOff {
        channel: u8,
        key: u8,
        velocity: u8,
    },
    NoteOn {
        channel: u8,
        key: u8,
        velocity: u8,
        duration_ticks: u32,
    },
    PolyPressure {
        channel: u8,
        key: u8,
        pressure: u8,
    },
    /// Includes extended Miles controls; no GM/control-flow interpretation.
    Controller {
        channel: u8,
        controller: u8,
        value: u8,
    },
    ProgramChange {
        channel: u8,
        program: u8,
    },
    ChannelPressure {
        channel: u8,
        pressure: u8,
    },
    PitchBend {
        channel: u8,
        lsb: u8,
        msb: u8,
    },
    /// Includes tempo and EOT. Payload bytes remain exact and uninterpreted.
    Meta {
        kind: u8,
        payload: Vec<u8>,
    },
    /// Preserve F0 versus F7 and the exact opaque payload.
    SysEx {
        status: u8,
        payload: Vec<u8>,
    },
}

#[derive(Default)]
struct Parser {
    chunks: usize,
    events: usize,
    opaque: Vec<XmiOpaqueChunk>,
}

struct Chunk<'a> {
    tag: [u8; 4],
    header_offset: usize,
    payload_offset: usize,
    payload: &'a [u8],
}
struct Chunks<'a> {
    bytes: &'a [u8],
    base: usize,
    position: usize,
}
impl<'a> Chunks<'a> {
    fn new(bytes: &'a [u8], base: usize) -> Self {
        Self {
            bytes,
            base,
            position: 0,
        }
    }
    fn next(
        &mut self,
        parser: &mut Parser,
        sequence: Option<XmiSequenceOrdinal>,
    ) -> Result<Option<Chunk<'a>>> {
        if self.position == self.bytes.len() {
            return Ok(None);
        }
        let header_offset = self.base + self.position;
        let fail = |message| error(header_offset, sequence, message);
        let remaining = &self.bytes[self.position..];
        let header = remaining
            .get(..8)
            .ok_or_else(|| fail("truncated chunk header"))?;
        let length = u32::from_be_bytes(header[4..8].try_into().unwrap()) as usize;
        let padded_length = length
            .checked_add(length % 2)
            .ok_or_else(|| fail("chunk length overflow"))?;
        let end = 8usize
            .checked_add(padded_length)
            .ok_or_else(|| fail("chunk length overflow"))?;
        if end > remaining.len() {
            return Err(fail("chunk payload or IFF padding exceeds parent"));
        }
        if parser.chunks == MAX_XMI_CHUNKS {
            return Err(fail("chunk count limit exceeded"));
        }
        parser.chunks += 1;
        self.position += end;
        Ok(Some(Chunk {
            tag: header[..4].try_into().unwrap(),
            header_offset,
            payload_offset: header_offset + 8,
            payload: &remaining[8..8 + length],
        }))
    }
}
impl Chunk<'_> {
    fn container(&self, expected: &[u8; 4], sequence: Option<XmiSequenceOrdinal>) -> Result<()> {
        if self.payload.get(..4) != Some(expected.as_slice()) {
            return Err(error(
                self.payload_offset,
                sequence,
                "unexpected or truncated container type",
            ));
        }
        Ok(())
    }
    fn children(&self) -> Chunks<'_> {
        Chunks::new(&self.payload[4..], self.payload_offset + 4)
    }
}

impl XmiFile {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_XMI_BYTES {
            return Err(error(0, None, "source byte limit exceeded"));
        }
        let mut parser = Parser::default();
        let mut chunks = Chunks::new(bytes, 0);
        let mut count = None;
        let mut sequences = None;
        while let Some(chunk) = chunks.next(&mut parser, None)? {
            match &chunk.tag {
                b"FORM" => {
                    if count.is_some() || sequences.is_some() {
                        return Err(error(
                            chunk.header_offset,
                            None,
                            "duplicate or misplaced XDIR",
                        ));
                    }
                    chunk.container(b"XDIR", None)?;
                    count = Some(parser.directory(&chunk)?);
                }
                b"CAT " => {
                    if sequences.is_some() || count.is_none() {
                        return Err(error(
                            chunk.header_offset,
                            None,
                            "duplicate or misplaced XMID catalog",
                        ));
                    }
                    chunk.container(b"XMID", None)?;
                    sequences = Some(parser.catalog(&chunk)?);
                }
                _ => parser.unknown(chunk, XmiChunkScope::File)?,
            }
        }
        let count = count.ok_or_else(|| error(0, None, "missing XDIR"))?;
        let sequences =
            sequences.ok_or_else(|| error(bytes.len(), None, "missing XMID catalog"))?;
        if count != sequences.len() {
            return Err(error(bytes.len(), None, "INFO sequence count mismatch"));
        }
        Ok(Self {
            sequences,
            opaque_chunks: parser.opaque,
        })
    }
}

impl Parser {
    fn unknown(&mut self, chunk: Chunk<'_>, scope: XmiChunkScope) -> Result<()> {
        // Supported nesting has exactly three levels. Treat unknown leaf data
        // opaquely, but never hide misplaced structural/known chunks inside it.
        if matches!(
            &chunk.tag,
            b"FORM" | b"CAT " | b"LIST" | b"INFO" | b"EVNT" | b"TIMB" | b"RBRN"
        ) {
            let sequence = match scope {
                XmiChunkScope::Sequence(ordinal) => Some(ordinal),
                _ => None,
            };
            return Err(error(
                chunk.header_offset,
                sequence,
                "chunk is invalid at this nesting level",
            ));
        }
        self.opaque.push(XmiOpaqueChunk {
            scope,
            tag: chunk.tag,
            header_offset: chunk.header_offset,
            payload: chunk.payload.to_vec(),
        });
        Ok(())
    }
    fn directory(&mut self, directory: &Chunk<'_>) -> Result<usize> {
        let mut chunks = directory.children();
        let mut count = None;
        while let Some(chunk) = chunks.next(self, None)? {
            if &chunk.tag == b"INFO" {
                if count.is_some() || chunk.payload.len() != 2 {
                    return Err(error(
                        chunk.payload_offset,
                        None,
                        "duplicate or invalid INFO",
                    ));
                }
                let value = usize::from(u16::from_le_bytes(chunk.payload.try_into().unwrap()));
                if value == 0 || value > MAX_XMI_SEQUENCES {
                    return Err(error(
                        chunk.payload_offset,
                        None,
                        "sequence count limit exceeded",
                    ));
                }
                count = Some(value);
            } else {
                self.unknown(chunk, XmiChunkScope::Directory)?;
            }
        }
        count.ok_or_else(|| error(directory.header_offset, None, "missing INFO"))
    }
    fn catalog(&mut self, catalog: &Chunk<'_>) -> Result<Vec<XmiSequence>> {
        let mut chunks = catalog.children();
        let mut sequences = Vec::new();
        while let Some(chunk) = chunks.next(self, None)? {
            if &chunk.tag == b"FORM" {
                if sequences.len() == MAX_XMI_SEQUENCES {
                    return Err(error(
                        chunk.header_offset,
                        None,
                        "sequence count limit exceeded",
                    ));
                }
                let ordinal = XmiSequenceOrdinal(sequences.len() as u16);
                chunk.container(b"XMID", Some(ordinal))?;
                sequences.push(self.sequence(&chunk, ordinal)?);
            } else {
                self.unknown(chunk, XmiChunkScope::Catalog)?;
            }
        }
        Ok(sequences)
    }
    fn sequence(&mut self, form: &Chunk<'_>, ordinal: XmiSequenceOrdinal) -> Result<XmiSequence> {
        let mut chunks = form.children();
        let mut timbres = None;
        let mut branches = None;
        let mut event_chunk = None;
        let mut branch_offset = 0;
        while let Some(chunk) = chunks.next(self, Some(ordinal))? {
            match &chunk.tag {
                b"TIMB" => {
                    if timbres.is_some() {
                        return Err(error(chunk.header_offset, Some(ordinal), "duplicate TIMB"));
                    }
                    timbres = Some(
                        table(&chunk, ordinal, 2)?
                            .chunks_exact(2)
                            .map(|pair| [pair[0], pair[1]])
                            .collect(),
                    );
                }
                b"RBRN" => {
                    if branches.is_some() {
                        return Err(error(chunk.header_offset, Some(ordinal), "duplicate RBRN"));
                    }
                    branch_offset = chunk.payload_offset + 2;
                    branches = Some(
                        table(&chunk, ordinal, 6)?
                            .chunks_exact(6)
                            .map(|record| XmiBranch {
                                marker_id: u16::from_le_bytes(record[..2].try_into().unwrap()),
                                event_offset: u32::from_le_bytes(record[2..].try_into().unwrap()),
                                event_index: 0,
                            })
                            .collect::<Vec<_>>(),
                    );
                }
                b"EVNT" => {
                    if event_chunk.is_some() {
                        return Err(error(chunk.header_offset, Some(ordinal), "duplicate EVNT"));
                    }
                    event_chunk = Some(chunk);
                }
                _ => self.unknown(chunk, XmiChunkScope::Sequence(ordinal))?,
            }
        }
        let chunk =
            event_chunk.ok_or_else(|| error(form.header_offset, Some(ordinal), "missing EVNT"))?;
        let (events, end_tick, has_eot_padding) = self.events(&chunk, ordinal)?;
        for (index, branch) in branches.iter_mut().flatten().enumerate() {
            branch.event_index = events
                .binary_search_by_key(&branch.event_offset, |event| event.offset)
                .map_err(|_| {
                    error(
                        branch_offset + index * 6 + 2,
                        Some(ordinal),
                        "branch target is not an event-start offset",
                    )
                })?;
        }
        Ok(XmiSequence {
            ordinal,
            form_offset: form.header_offset,
            evnt_payload_offset: chunk.payload_offset,
            evnt_payload_len: chunk.payload.len() as u32,
            timbres,
            branches,
            events,
            end_tick,
            has_eot_padding,
        })
    }
    fn events(
        &mut self,
        chunk: &Chunk<'_>,
        ordinal: XmiSequenceOrdinal,
    ) -> Result<(Vec<XmiEvent>, u64, bool)> {
        let mut reader = EventReader {
            bytes: chunk.payload,
            position: 0,
            base: chunk.payload_offset,
            ordinal,
        };
        let mut events = Vec::new();
        let mut tick = 0u64;
        loop {
            if reader.position == reader.bytes.len() {
                return Err(reader.error("missing end-of-track event"));
            }
            if events.len() == MAX_XMI_SEQUENCE_EVENTS || self.events == MAX_XMI_FILE_EVENTS {
                return Err(reader.error("event count limit exceeded"));
            }
            let offset = reader.position;
            let mut delay_ticks = 0u64;
            let mut has_zero_delay_byte = false;
            while reader
                .bytes
                .get(reader.position)
                .is_some_and(|byte| *byte < 0x80)
            {
                let delay = reader.byte()?;
                has_zero_delay_byte |= delay == 0;
                delay_ticks =
                    checked_ticks(delay_ticks, u64::from(delay), reader.base + offset, ordinal)?;
            }
            tick = checked_ticks(tick, delay_ticks, reader.base + offset, ordinal)?;
            let status_offset = reader.position;
            let status = reader.byte()?;
            let channel = status & 0x0f;
            let mut eot = false;
            let kind = match status >> 4 {
                0x8 => XmiEventKind::NoteOff {
                    channel,
                    key: reader.data_byte()?,
                    velocity: reader.data_byte()?,
                },
                0x9 => {
                    let key = reader.data_byte()?;
                    let velocity = reader.data_byte()?;
                    let duration_ticks = reader.vlq()?;
                    checked_ticks(
                        tick,
                        u64::from(duration_ticks),
                        reader.base + status_offset,
                        ordinal,
                    )?;
                    XmiEventKind::NoteOn {
                        channel,
                        key,
                        velocity,
                        duration_ticks,
                    }
                }
                0xa => XmiEventKind::PolyPressure {
                    channel,
                    key: reader.data_byte()?,
                    pressure: reader.data_byte()?,
                },
                0xb => XmiEventKind::Controller {
                    channel,
                    controller: reader.data_byte()?,
                    value: reader.data_byte()?,
                },
                0xc => XmiEventKind::ProgramChange {
                    channel,
                    program: reader.data_byte()?,
                },
                0xd => XmiEventKind::ChannelPressure {
                    channel,
                    pressure: reader.data_byte()?,
                },
                0xe => XmiEventKind::PitchBend {
                    channel,
                    lsb: reader.data_byte()?,
                    msb: reader.data_byte()?,
                },
                0xf => match status {
                    0xff => {
                        let kind = reader.byte()?;
                        let payload = reader.payload()?;
                        eot = kind == 0x2f;
                        if eot && !payload.is_empty() {
                            return Err(reader.error("end-of-track payload is not empty"));
                        }
                        XmiEventKind::Meta { kind, payload }
                    }
                    0xf0 | 0xf7 => XmiEventKind::SysEx {
                        status,
                        payload: reader.payload()?,
                    },
                    _ => return Err(reader.error("unsupported system status")),
                },
                _ => return Err(reader.error("missing explicit status")),
            };
            events.push(XmiEvent {
                offset: offset as u32,
                status_offset: status_offset as u32,
                delay_ticks,
                has_zero_delay_byte,
                tick,
                kind,
            });
            self.events += 1;
            if eot {
                let padding = match &reader.bytes[reader.position..] {
                    [] => false,
                    [0] => true,
                    _ => return Err(reader.error("unexpected data after end-of-track")),
                };
                return Ok((events, tick, padding));
            }
        }
    }
}

fn table<'a>(chunk: &Chunk<'a>, ordinal: XmiSequenceOrdinal, stride: usize) -> Result<&'a [u8]> {
    let fail = |message| error(chunk.payload_offset, Some(ordinal), message);
    let count = chunk
        .payload
        .get(..2)
        .ok_or_else(|| fail("truncated table count"))?;
    let count = usize::from(u16::from_le_bytes(count.try_into().unwrap()));
    if count > MAX_XMI_TABLE_RECORDS {
        return Err(fail("table record limit exceeded"));
    }
    if chunk.payload.len() != 2 + count * stride {
        return Err(fail("table length/count mismatch"));
    }
    Ok(&chunk.payload[2..])
}
fn checked_ticks(tick: u64, delay: u64, offset: usize, ordinal: XmiSequenceOrdinal) -> Result<u64> {
    tick.checked_add(delay)
        .ok_or_else(|| error(offset, Some(ordinal), "tick arithmetic overflow"))
}
struct EventReader<'a> {
    bytes: &'a [u8],
    position: usize,
    base: usize,
    ordinal: XmiSequenceOrdinal,
}
impl EventReader<'_> {
    fn error(&self, message: &'static str) -> XmiError {
        error(self.base + self.position, Some(self.ordinal), message)
    }
    fn byte(&mut self) -> Result<u8> {
        let byte = *self
            .bytes
            .get(self.position)
            .ok_or_else(|| self.error("truncated event"))?;
        self.position += 1;
        Ok(byte)
    }
    fn data_byte(&mut self) -> Result<u8> {
        let byte = self.byte()?;
        if byte >= 0x80 {
            return Err(self.error("channel data byte has status bit"));
        }
        Ok(byte)
    }
    fn vlq(&mut self) -> Result<u32> {
        let mut value = 0;
        for _ in 0..4 {
            let byte = self.byte()?;
            value = (value << 7) | u32::from(byte & 0x7f);
            if byte < 0x80 {
                return Ok(value);
            }
        }
        Err(self.error("VLQ exceeds four bytes"))
    }
    fn payload(&mut self) -> Result<Vec<u8>> {
        let length = self.vlq()? as usize;
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| self.error("event payload length overflow"))?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| self.error("event payload exceeds EVNT"))?;
        self.position = end;
        Ok(bytes.to_vec())
    }
}

// Keep parser-only tests separate from runtime audio tests: none can open a device.
#[cfg(test)]
#[path = "xmi_tests.rs"]
mod tests;
