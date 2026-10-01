//! Bounded offline synthesis worker. The output callback only consumes PCM.
use super::schedule::{MAX_SOURCE_EVENTS_PER_TICK, SampleClock, XmiScheduler};
use crate::audio::{
    decode::MusicStream,
    midi_synth::{MAX_RENDER_FRAMES, MAX_SYSEX_BYTES, MidiSynth},
};
use anyhow::{Result, ensure};
use openeq_assets::audio::xmi::{XmiFile, XmiSequenceOrdinal};
use std::{
    num::NonZero,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

const SAMPLE_RATE: u32 = 48_000;
const MAX_SECONDS: u64 = 30 * 60;
// Finite release tail is a client policy, not a native Miles timing claim.
const RELEASE_SECONDS: u64 = 2;
/// Includes metadata, consumed controls and releases across the whole PCM block.
const MAX_BLOCK_EVENTS: usize = MAX_SOURCE_EVENTS_PER_TICK * 2;
static WORKERS: OnceLock<Arc<AtomicUsize>> = OnceLock::new();
#[derive(Debug)]
pub(crate) struct Busy;
impl std::fmt::Display for Busy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("two XMI workers are still active")
    }
}
impl std::error::Error for Busy {}
struct WorkerSlot(Arc<AtomicUsize>);
impl WorkerSlot {
    fn acquire(workers: Arc<AtomicUsize>) -> Result<Self> {
        workers
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < 2).then_some(count + 1)
            })
            .map(|_| Self(workers))
            .map_err(|_| Busy.into())
    }
}
impl Drop for WorkerSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

pub fn stream(bytes: Vec<u8>, ordinal: XmiSequenceOrdinal) -> Result<MusicStream> {
    stream_with(
        bytes,
        ordinal,
        Arc::clone(WORKERS.get_or_init(Arc::default)),
        || Ok(MidiSynth::create(SAMPLE_RATE)?),
    )
}

// The factory crosses the thread boundary; the synthesizer never does. Keeping
// this seam generic also permits deterministic tests without initializing an AU.
trait RenderSynth {
    fn midi_event(&mut self, status: u8, data1: u8, data2: u8) -> Result<()>;
    fn sysex(&mut self, packet: &[u8]) -> Result<()>;
    fn render(&mut self, samples: &mut [f32]) -> Result<()>;
}
impl RenderSynth for MidiSynth {
    fn midi_event(&mut self, status: u8, data1: u8, data2: u8) -> Result<()> {
        Ok(MidiSynth::midi_event(self, status, data1, data2)?)
    }
    fn sysex(&mut self, packet: &[u8]) -> Result<()> {
        Ok(MidiSynth::sysex(self, packet)?)
    }
    fn render(&mut self, samples: &mut [f32]) -> Result<()> {
        Ok(MidiSynth::render(self, samples)?)
    }
}

fn stream_with<S: RenderSynth + 'static>(
    bytes: Vec<u8>,
    ordinal: XmiSequenceOrdinal,
    workers: Arc<AtomicUsize>,
    create: impl FnOnce() -> Result<S> + Send + 'static,
) -> Result<MusicStream> {
    let slot = WorkerSlot::acquire(workers)?;
    let file = XmiFile::parse(&bytes)?;
    let sequence = file
        .sequences
        .into_iter()
        .nth(ordinal.index())
        .ok_or_else(|| anyhow::anyhow!("XMI sequence {} is absent", ordinal.0))?;
    let clock = SampleClock::miles_default(SAMPLE_RATE)?;
    let mut schedule = XmiScheduler::new(Arc::new(sequence), clock)?;
    let known_end = schedule.known_end_frame();
    let end = known_end.unwrap_or(MAX_SECONDS * u64::from(SAMPLE_RATE));
    ensure!(
        end <= MAX_SECONDS * u64::from(SAMPLE_RATE),
        "XMI sequence exceeds 30 minute limit"
    );
    let (sender, receiver) = mpsc::sync_channel(8);
    let failure = Arc::new(AtomicBool::new(false));
    let worker_failure = Arc::clone(&failure);
    std::thread::Builder::new()
        .name("openeq-xmi-synth".into())
        .spawn(move || {
            let _slot = slot;
            // The AudioUnit is deliberately !Send: it is owned only by this worker.
            let mut synth = match create() {
                Ok(synth) => synth,
                Err(error) => {
                    worker_failure.store(true, Ordering::Release);
                    tracing::warn!(%error, "XMI synthesizer unavailable");
                    return;
                }
            };
            let result = render(&mut schedule, &mut synth, end, |block| {
                sender.send(block).is_ok()
            });
            if let Err(error) = result {
                worker_failure.store(true, Ordering::Release);
                tracing::warn!(%error, "XMI synthesis stopped");
            }
            // Dropping receiver (zone/track cancellation) disconnects a blocked
            // sender, bounds remaining work to one block, and disposes the unit here.
        })?;
    Ok(MusicStream::from_queue(
        receiver,
        NonZero::new(2).unwrap(),
        NonZero::new(SAMPLE_RATE).unwrap(),
        known_end.map(|end| {
            Duration::from_secs_f64(end as f64 / f64::from(SAMPLE_RATE) + RELEASE_SECONDS as f64)
        }),
        failure,
    ))
}

fn render(
    schedule: &mut XmiScheduler,
    synth: &mut impl RenderSynth,
    end: u64,
    mut publish: impl FnMut(Vec<f32>) -> bool,
) -> Result<()> {
    let result = render_inner(schedule, synth, end, &mut publish);
    // Receiver cancellation, policy cutoff and runtime guard errors must all
    // stop the active native-style note slots before the worker is disposed.
    let cleanup = cancel_schedule(schedule, synth);
    result.and(cleanup)
}

fn cancel_schedule(schedule: &mut XmiScheduler, synth: &mut impl RenderSynth) -> Result<()> {
    let mut cleanup_error = None;
    for message in schedule.cancel() {
        if let Err(error) = synth.midi_event(message.status, message.data1, message.data2) {
            cleanup_error.get_or_insert(error);
        }
    }
    cleanup_error.map_or(Ok(()), Err)
}

fn render_inner(
    schedule: &mut XmiScheduler,
    synth: &mut impl RenderSynth,
    limit: u64,
    publish: &mut impl FnMut(Vec<f32>) -> bool,
) -> Result<()> {
    let mut sysex = [0u8; MAX_SYSEX_BYTES];
    sysex[0] = 0xf0;
    let mut frame = 0;
    let tail = RELEASE_SECONDS * u64::from(SAMPLE_RATE);
    let mut final_frame = limit
        .checked_add(tail)
        .ok_or_else(|| anyhow::anyhow!("XMI stream limit overflow"))?;
    let mut ending = false;
    let mut block_events = 0;
    let mut block = Vec::with_capacity(MAX_RENDER_FRAMES * 2);
    while frame < final_frame {
        while schedule.next_frame()?.is_some_and(|next| next <= frame) {
            ensure!(
                block_events < MAX_BLOCK_EVENTS,
                "XMI render event-work limit exceeded"
            );
            block_events += 1;
            let Some(event) = schedule.next_event()? else {
                break;
            };
            if let Some(message) = event.midi_message() {
                synth.midi_event(message.status, message.data1, message.data2)?;
            } else if let Some(payload) = schedule.sysex_payload(event) {
                // Preflight bounded this complete packet; reuse fixed storage
                // rather than allocate on every loop iteration.
                sysex[1..=payload.len()].copy_from_slice(payload);
                synth.sysex(&sysex[..=payload.len()])?;
            }
        }
        if !ending && (schedule.is_finished() || frame >= limit) {
            if !schedule.is_finished() {
                tracing::warn!("XMI loop stream reached its 30 minute playback limit");
                cancel_schedule(schedule, synth)?;
            }
            ending = true;
            final_frame = frame
                .checked_add(tail)
                .ok_or_else(|| anyhow::anyhow!("XMI release-tail overflow"))?;
        }
        let boundary = if ending {
            final_frame
        } else {
            schedule.next_frame()?.unwrap_or(limit).min(limit)
        };
        ensure!(boundary > frame, "XMI sample timeline made no progress");
        let room = MAX_RENDER_FRAMES - block.len() / 2;
        let frames = (boundary - frame).min(room as u64) as usize;
        let begin = block.len();
        block.resize(begin + frames * 2, 0.);
        synth.render(&mut block[begin..])?;
        frame += frames as u64;
        if block.len() == MAX_RENDER_FRAMES * 2 || frame == final_frame {
            if !publish(block) {
                return Ok(());
            }
            block = Vec::with_capacity(MAX_RENDER_FRAMES * 2);
            block_events = 0;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
