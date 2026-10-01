//! Bounded Miles-compatible event ordering, without synthesis or a device.
//!
//! Native evidence: `docs/XMI_NATIVE_SELECTION.md` and `docs/XMI_NATIVE_LOOPS.md`.
//! Each tick expires notes in
//! ascending slot order before authored events. Zero-duration notes expire on
//! the next tick. Same-key overlap retains independent slots and unconditional
//! MIDI releases; no modern voice-stealing policy is substituted.
use crate::audio::midi_synth::validate_sysex_payload;
use openeq_assets::audio::xmi::{
    DEFAULT_TICKS_PER_SECOND, MAX_XMI_SEQUENCE_EVENTS, XmiEventKind, XmiSequence,
};
use std::{cmp::Reverse, collections::BinaryHeap, fmt, sync::Arc};

pub const MAX_ACTIVE_NOTES: usize = 32;
pub const MAX_BATCH_EVENTS: usize = 512;
pub const MAX_DIAGNOSTICS: usize = 32;
/// Includes controls/metadata that produce no MIDI. This persists across pull
/// batches so a caller cannot accidentally spin forever at one execution tick.
pub const MAX_SOURCE_EVENTS_PER_TICK: usize = MAX_XMI_SEQUENCE_EVENTS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SampleClock {
    pub sample_rate: u32,
    pub ticks_per_second: u32,
}
impl SampleClock {
    pub fn miles_default(sample_rate: u32) -> Result<Self, ScheduleError> {
        Self::new(sample_rate, DEFAULT_TICKS_PER_SECOND)
    }
    pub fn new(sample_rate: u32, ticks_per_second: u32) -> Result<Self, ScheduleError> {
        if sample_rate == 0 || ticks_per_second == 0 {
            return Err(ScheduleError {
                event_index: None,
                issue: ScheduleIssue::InvalidClock,
            });
        }
        Ok(Self {
            sample_rate,
            ticks_per_second,
        })
    }
    /// Absolute rational conversion avoids accumulating per-tick rounding.
    /// Samples start at floor(tick * rate / clock); equal frames retain ticks
    /// and source order, even when the output sample rate is below the clock.
    pub fn frame_at_tick(self, tick: u64) -> Result<u64, ScheduleError> {
        if self.sample_rate == 0 || self.ticks_per_second == 0 {
            return Err(ScheduleError {
                event_index: None,
                issue: ScheduleIssue::InvalidClock,
            });
        }
        u64::try_from(
            u128::from(tick) * u128::from(self.sample_rate) / u128::from(self.ticks_per_second),
        )
        .map_err(|_| ScheduleError {
            event_index: None,
            issue: ScheduleIssue::FrameOverflow,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleIssue {
    InvalidClock,
    FrameOverflow,
    EventLimit,
    InvalidSequence,
    InvalidChannelData,
    UnsupportedController(u8),
    UnsupportedSysEx,
    UnsupportedZeroDelay,
    TickOverflow,
    ActiveNoteLimit,
    BatchLimit,
    UnmatchedLoopBreak,
    SameTickWorkLimit,
    IdentityOverflow,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduleError {
    pub event_index: Option<usize>,
    pub issue: ScheduleIssue,
}
impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "XMI scheduling {:?} at source event {:?}",
            self.issue, self.event_index
        )
    }
}
impl std::error::Error for ScheduleError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduleDiagnostic {
    pub first_event: Option<usize>,
    pub issue: ScheduleIssue,
    pub occurrences: usize,
}
#[derive(Clone, Debug, Default)]
pub struct PreflightReport {
    pub diagnostics: Vec<ScheduleDiagnostic>,
    pub suppressed: usize,
    pub peak_active_notes: usize,
    /// Exact for straight-through sequences only. Loop execution can finish
    /// later or remain indefinite; its clock and note bounds are checked live.
    pub end_frame: Option<u64>,
}
impl PreflightReport {
    pub fn is_supported(&self) -> bool {
        self.diagnostics.is_empty() && self.suppressed == 0
    }
    fn add(&mut self, event_index: Option<usize>, issue: ScheduleIssue) {
        if let Some(existing) = self
            .diagnostics
            .iter_mut()
            .find(|entry| entry.issue == issue)
        {
            existing.occurrences += 1;
        } else if self.diagnostics.len() < MAX_DIAGNOSTICS {
            self.diagnostics.push(ScheduleDiagnostic {
                first_event: event_index,
                issue,
                occurrences: 1,
            });
        } else {
            self.suppressed += 1;
        }
    }
}

/// Audits all bounded source events before emitting anything. Branch tables
/// alone are passive metadata. Their executing controller109 and the unproven
/// extended controls are rejected, not flattened or forwarded as GM commands.
pub fn preflight(sequence: &XmiSequence, clock: SampleClock) -> PreflightReport {
    let mut report = PreflightReport::default();
    match clock.frame_at_tick(sequence.end_tick) {
        Ok(frame) => report.end_frame = Some(frame),
        Err(error) => report.add(None, error.issue),
    }
    if sequence.events.len() > MAX_XMI_SEQUENCE_EVENTS {
        report.add(None, ScheduleIssue::EventLimit);
        return report;
    }
    let has_loops = sequence.events.iter().any(|event| {
        matches!(
            event.kind,
            XmiEventKind::Controller {
                controller: 116 | 117,
                ..
            }
        )
    });
    let mut active = [None::<u64>; MAX_ACTIVE_NOTES];
    let mut previous_tick = 0u64;
    let mut eot = false;
    for (index, event) in sequence.events.iter().enumerate() {
        if event.has_zero_delay_byte {
            report.add(Some(index), ScheduleIssue::UnsupportedZeroDelay);
        }
        if eot
            || previous_tick.checked_add(event.delay_ticks) != Some(event.tick)
            || event.tick > sequence.end_tick
        {
            report.add(Some(index), ScheduleIssue::InvalidSequence);
        }
        previous_tick = event.tick;
        for note in &mut active {
            if note.is_some_and(|end| end <= event.tick) {
                *note = None;
            }
        }
        if midi_message(&event.kind).is_err() {
            report.add(Some(index), ScheduleIssue::InvalidChannelData);
        }
        match &event.kind {
            XmiEventKind::NoteOn { duration_ticks, .. } => {
                if let Some(end) = event.tick.checked_add(u64::from((*duration_ticks).max(1))) {
                    if let Some(slot) = active.iter_mut().find(|slot| slot.is_none()) {
                        *slot = Some(end);
                        report.peak_active_notes = report
                            .peak_active_notes
                            .max(active.iter().flatten().count());
                    } else if !has_loops {
                        report.add(Some(index), ScheduleIssue::ActiveNoteLimit);
                    }
                } else {
                    report.add(Some(index), ScheduleIssue::TickOverflow);
                }
            }
            XmiEventKind::Controller { controller, .. }
                if matches!(controller, 106 | 109 | 110 | 111 | 115 | 118 | 119) =>
            {
                report.add(
                    Some(index),
                    ScheduleIssue::UnsupportedController(*controller),
                );
            }
            XmiEventKind::SysEx { status, payload }
                if validate_sysex_payload(*status, payload).is_err() =>
            {
                report.add(Some(index), ScheduleIssue::UnsupportedSysEx);
            }
            XmiEventKind::Meta {
                kind: 0x2f,
                payload,
            } => {
                if !payload.is_empty() || event.tick != sequence.end_tick {
                    report.add(Some(index), ScheduleIssue::InvalidSequence);
                }
                eot = true;
            }
            _ => {}
        }
    }
    if !eot {
        report.add(None, ScheduleIssue::InvalidSequence);
    }
    if has_loops {
        // This occupancy estimate describes only the source's linear pass.
        // It cannot certify execution across repeats or predict its endpoint.
        report.end_frame = None;
    }
    report
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MidiMessage {
    pub status: u8,
    pub data1: u8,
    /// Zero for the one-data-byte program/channel-pressure shapes.
    pub data2: u8,
}
fn midi_message(kind: &XmiEventKind) -> Result<Option<MidiMessage>, ()> {
    let (status, channel, data1, data2) = match *kind {
        XmiEventKind::NoteOff {
            channel,
            key,
            velocity,
        } => (0x80, channel, key, velocity),
        XmiEventKind::NoteOn {
            channel,
            key,
            velocity,
            ..
        } => (0x90, channel, key, velocity),
        XmiEventKind::PolyPressure {
            channel,
            key,
            pressure,
        } => (0xa0, channel, key, pressure),
        XmiEventKind::Controller {
            channel,
            controller,
            value,
        } => (0xb0, channel, controller, value),
        XmiEventKind::ProgramChange { channel, program } => (0xc0, channel, program, 0),
        XmiEventKind::ChannelPressure { channel, pressure } => (0xd0, channel, pressure, 0),
        XmiEventKind::PitchBend { channel, lsb, msb } => (0xe0, channel, lsb, msb),
        XmiEventKind::Meta { .. } | XmiEventKind::SysEx { .. } => return Ok(None),
    };
    if channel > 15 || data1 > 127 || data2 > 127 {
        return Err(());
    }
    Ok(Some(MidiMessage {
        status: status | channel,
        data1,
        data2,
    }))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NoteIdentity {
    pub source_event: usize,
    pub slot: u8,
    /// Distinguishes repeated visits to a source event, even after slot reuse.
    pub occurrence: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduledKind {
    /// Complete F0 packet retained in the immutable source. Access through
    /// XmiScheduler::sysex_payload; its trailing F7 is part of that payload.
    SysEx {
        event_index: usize,
    },
    /// Metadata stays observable by source index but is not MIDI output. In
    /// particular, tempo does not rewrite the XMI tick clock.
    Source {
        event_index: usize,
        message: Option<MidiMessage>,
        note: Option<NoteIdentity>,
    },
    Release {
        note: NoteIdentity,
        message: MidiMessage,
    },
    Cleanup {
        note: Option<NoteIdentity>,
        message: MidiMessage,
    },
    End,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScheduledEvent {
    pub tick: u64,
    pub frame: u64,
    pub kind: ScheduledKind,
}
impl ScheduledEvent {
    pub fn midi_message(self) -> Option<MidiMessage> {
        match self.kind {
            ScheduledKind::Source { message, .. } => message,
            ScheduledKind::Release { message, .. } | ScheduledKind::Cleanup { message, .. } => {
                Some(message)
            }
            ScheduledKind::End | ScheduledKind::SysEx { .. } => None,
        }
    }
}
#[derive(Clone, Copy, Debug)]
struct ActiveNote {
    id: NoteIdentity,
    channel: u8,
    key: u8,
}
impl ActiveNote {
    fn release(self) -> MidiMessage {
        MidiMessage {
            status: 0x80 | self.channel,
            data1: self.key,
            data2: 0,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Release {
    tick: u64,
    slot: u8,
    id: NoteIdentity,
}

#[derive(Clone, Copy, Debug)]
struct LoopSlot {
    start: usize,
    remaining: u8,
}

pub struct XmiScheduler {
    sequence: Arc<XmiSequence>,
    clock: SampleClock,
    source_index: usize,
    source_tick: u64,
    known_end_frame: Option<u64>,
    loops: [Option<LoopSlot>; 4],
    work_tick: u64,
    source_work: usize,
    next_note_id: u64,
    active: [Option<ActiveNote>; MAX_ACTIVE_NOTES],
    releases: BinaryHeap<Reverse<Release>>,
    sustain: [u8; 16],
    ending: bool,
    finished: bool,
    failure: Option<ScheduleError>,
}
impl XmiScheduler {
    pub fn new(sequence: Arc<XmiSequence>, clock: SampleClock) -> Result<Self, ScheduleError> {
        let report = preflight(&sequence, clock);
        if let Some(issue) = report.diagnostics.first() {
            return Err(ScheduleError {
                event_index: issue.first_event,
                issue: issue.issue,
            });
        }
        Ok(Self {
            source_tick: sequence.events[0].tick,
            known_end_frame: report.end_frame,
            sequence,
            clock,
            source_index: 0,
            loops: [None; 4],
            work_tick: 0,
            source_work: 0,
            next_note_id: 0,
            active: [None; MAX_ACTIVE_NOTES],
            releases: BinaryHeap::with_capacity(MAX_ACTIVE_NOTES),
            sustain: [0; 16],
            ending: false,
            finished: false,
            failure: None,
        })
    }
    pub fn is_finished(&self) -> bool {
        self.finished
    }
    pub fn known_end_frame(&self) -> Option<u64> {
        self.known_end_frame
    }
    /// Payload for a SysEx event produced by this scheduler. It excludes F0
    /// and includes F7; preflight has checked framing and the native size bound.
    pub fn sysex_payload(&self, event: ScheduledEvent) -> Option<&[u8]> {
        let ScheduledKind::SysEx { event_index } = event.kind else {
            return None;
        };
        match &self.sequence.events.get(event_index)?.kind {
            XmiEventKind::SysEx {
                status: 0xf0,
                payload,
            } => Some(payload),
            _ => None,
        }
    }
    /// Useful for splitting a bounded synthesis block exactly at an event.
    pub fn next_frame(&self) -> Result<Option<u64>, ScheduleError> {
        if self.finished {
            return Ok(None);
        }
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let tick = if self.ending {
            self.source_tick
        } else {
            self.releases.peek().map_or(self.source_tick, |release| {
                release.0.tick.min(self.source_tick)
            })
        };
        self.clock.frame_at_tick(tick).map(Some)
    }
    pub fn next_batch(&mut self, max_events: usize) -> Result<Vec<ScheduledEvent>, ScheduleError> {
        if max_events == 0 || max_events > MAX_BATCH_EVENTS {
            return Err(ScheduleError {
                event_index: None,
                issue: ScheduleIssue::BatchLimit,
            });
        }
        let mut events = Vec::with_capacity(max_events);
        for _ in 0..max_events {
            let Some(event) = self.next_event()? else {
                break;
            };
            events.push(event);
        }
        Ok(events)
    }
    /// One call performs at most one source command or note release. Consumed
    /// loop controls remain visible as metadata, so even control-only loops
    /// yield to bounded batches and can be cancelled between calls.
    pub fn next_event(&mut self) -> Result<Option<ScheduledEvent>, ScheduleError> {
        if self.finished {
            return Ok(None);
        }
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result = self.next_event_checked();
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }
    fn next_event_checked(&mut self) -> Result<Option<ScheduledEvent>, ScheduleError> {
        let frame = self
            .next_frame()?
            .expect("unfinished scheduler has an event");
        let (tick, kind) = if self.ending {
            (
                self.source_tick,
                self.cleanup_event().unwrap_or_else(|| {
                    self.finished = true;
                    ScheduledKind::End
                }),
            )
        } else if self
            .releases
            .peek()
            .is_some_and(|release| release.0.tick <= self.source_tick)
        {
            let release = self.releases.pop().unwrap().0;
            let note = self.active[usize::from(release.slot)].take().unwrap();
            debug_assert_eq!(note.id, release.id);
            (
                release.tick,
                ScheduledKind::Release {
                    note: note.id,
                    message: note.release(),
                },
            )
        } else {
            let index = self.source_index;
            let tick = self.source_tick;
            let fail = |issue| ScheduleError {
                event_index: Some(index),
                issue,
            };
            if self.work_tick != tick {
                self.work_tick = tick;
                self.source_work = 0;
            }
            if self.source_work == MAX_SOURCE_EVENTS_PER_TICK {
                return Err(fail(ScheduleIssue::SameTickWorkLimit));
            }
            self.source_work += 1;
            let event = &self.sequence.events[index];
            let mut message =
                midi_message(&event.kind).map_err(|_| fail(ScheduleIssue::InvalidChannelData))?;
            let mut identity = None;
            let mut jump = None;
            match event.kind {
                XmiEventKind::NoteOn {
                    channel,
                    key,
                    duration_ticks,
                    ..
                } => {
                    let slot = self
                        .active
                        .iter()
                        .position(Option::is_none)
                        .ok_or_else(|| fail(ScheduleIssue::ActiveNoteLimit))?;
                    let id = NoteIdentity {
                        source_event: index,
                        slot: slot as u8,
                        occurrence: self.next_note_id,
                    };
                    self.next_note_id = self
                        .next_note_id
                        .checked_add(1)
                        .ok_or_else(|| fail(ScheduleIssue::IdentityOverflow))?;
                    let release_tick = tick
                        .checked_add(u64::from(duration_ticks.max(1)))
                        .ok_or_else(|| fail(ScheduleIssue::TickOverflow))?;
                    self.active[slot] = Some(ActiveNote { id, channel, key });
                    self.releases.push(Reverse(Release {
                        tick: release_tick,
                        slot: slot as u8,
                        id,
                    }));
                    identity = Some(id);
                }
                XmiEventKind::Controller {
                    controller: 116,
                    value,
                    ..
                } => {
                    message = None;
                    if let Some(slot) = self.loops.iter_mut().find(|slot| slot.is_none()) {
                        // The native cursor saves CC116 itself, not its body.
                        *slot = Some(LoopSlot {
                            start: index,
                            remaining: value,
                        });
                    }
                }
                XmiEventKind::Controller {
                    controller: 117,
                    value,
                    ..
                } => {
                    message = None;
                    let slot = self.loops.iter().rposition(Option::is_some);
                    match slot {
                        None if value < 64 => return Err(fail(ScheduleIssue::UnmatchedLoopBreak)),
                        None => {}
                        Some(slot) => {
                            let state = self.loops[slot].as_mut().unwrap();
                            if value < 64 || state.remaining == 1 {
                                self.loops[slot] = None;
                            } else {
                                if state.remaining != 0 {
                                    state.remaining -= 1;
                                }
                                jump = Some(state.start);
                            }
                        }
                    }
                }
                XmiEventKind::Controller {
                    channel,
                    controller: 64,
                    value,
                } => {
                    self.sustain[usize::from(channel)] = value;
                }
                XmiEventKind::Meta { kind: 0x2f, .. } => {
                    self.ending = true;
                    self.releases.clear();
                }
                _ => {}
            }
            if let Some(target) = jump {
                self.source_index = target;
                // Status-byte reentry excludes the start marker's preceding
                // delay. The first body event resumes on this same tick.
            } else {
                self.source_index += 1;
                if !self.ending {
                    self.source_tick = tick
                        .checked_add(self.sequence.events[self.source_index].delay_ticks)
                        .ok_or_else(|| fail(ScheduleIssue::TickOverflow))?;
                }
            }
            (
                tick,
                if matches!(event.kind, XmiEventKind::SysEx { .. }) {
                    ScheduledKind::SysEx { event_index: index }
                } else {
                    ScheduledKind::Source {
                        event_index: index,
                        message,
                        note: identity,
                    }
                },
            )
        };
        Ok(Some(ScheduledEvent { tick, frame, kind }))
    }
    fn cleanup_event(&mut self) -> Option<ScheduledKind> {
        if let Some(slot) = self.active.iter().position(Option::is_some) {
            let note = self.active[slot].take().unwrap();
            return Some(ScheduledKind::Cleanup {
                note: Some(note.id),
                message: note.release(),
            });
        }
        if let Some(channel) = self.sustain.iter().position(|value| *value >= 64) {
            self.sustain[channel] = 0;
            return Some(ScheduledKind::Cleanup {
                note: None,
                message: MidiMessage {
                    status: 0xb0 | channel as u8,
                    data1: 64,
                    data2: 0,
                },
            });
        }
        None
    }
    /// Caller emits these <=48 messages at its current render frame. This drops
    /// all pending source/releases and is idempotent; no old note can reappear.
    /// Cleanup follows native stop: active slots ascending, then held pedals.
    pub fn cancel(&mut self) -> Vec<MidiMessage> {
        self.finished = true;
        self.releases.clear();
        self.loops.fill(None);
        self.failure = None;
        self.source_index = self.sequence.events.len();
        let mut messages = Vec::with_capacity(MAX_ACTIVE_NOTES + 16);
        while let Some(ScheduledKind::Cleanup { message, .. }) = self.cleanup_event() {
            messages.push(message);
        }
        messages
    }
}

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "schedule_loop_tests.rs"]
mod loop_tests;

#[cfg(test)]
#[path = "schedule_sysex_tests.rs"]
mod sysex_tests;
