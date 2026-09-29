//! Opt-in GPU pass timestamps. Four asynchronous readbacks bound memory and
//! let rendering continue when the GPU has not completed an older sample.
use std::sync::{
    Arc,
    atomic::{AtomicU8, AtomicU32, Ordering},
};

const SLOTS: usize = 4;
const QUERY_COUNT: u32 = 14;
const QUERY_BYTES: u64 = QUERY_COUNT as u64 * 8;

/// GPU completion-boundary attribution, in milliseconds. An absent pass is
/// zero. Each pass is charged only the interval after the preceding passes'
/// completion, excluding overlapping vertex/fragment work on tile-based GPUs.
/// These are not isolated pass cycle counts. Transparency combines two passes;
/// total sums attribution, while frame_span also includes gaps between passes.
#[derive(Debug, Clone, Copy, Default)]
pub struct GpuFrameTimings {
    pub frame_id: u64,
    pub shadow_ms: f64,
    pub gbuffer_ms: f64,
    pub lighting_ms: f64,
    pub transparency_ms: f64,
    pub particles_ms: f64,
    pub ui_ms: f64,
    pub total_ms: f64,
    pub frame_span_ms: f64,
    /// Raw start-vertex to end-fragment intervals in order: shadow, G-buffer,
    /// lighting, transparency accumulation, transparency resolve, particles, UI.
    pub raw_pass_ms: [f64; 7],
    pub raw_pass_sum_ms: f64,
    pub overlap_ms: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GpuProfileStats {
    pub supported: bool,
    pub enabled: bool,
    pub submitted: u64,
    pub completed: u64,
    /// Frames rendered without a sample because all readback slots were busy.
    pub dropped: u64,
    pub failed: u64,
    pub in_flight: usize,
    pub latest: Option<GpuFrameTimings>,
}

#[derive(Clone, Copy)]
pub(crate) enum Pass {
    Shadow = 0,
    Gbuffer = 1,
    Lighting = 2,
    TransparencyAccumulate = 3,
    TransparencyResolve = 4,
    Particles = 5,
    Ui = 6,
}

struct Pending {
    status: Arc<AtomicU8>,
    awaiting_gpu: bool,
    frame_id: u64,
    mask: u32,
}

struct Slot {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    pending: Option<Pending>,
}

pub(crate) struct GpuProfiler {
    slots: [Slot; SLOTS],
    active: Option<usize>,
    mask: AtomicU32,
    next_frame: u64,
    period_ns: f64,
    stats: GpuProfileStats,
}

impl GpuProfiler {
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let slots = std::array::from_fn(|_| Slot {
            queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("frame GPU timings"),
                ty: wgpu::QueryType::Timestamp,
                count: QUERY_COUNT,
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("GPU timing query resolve"),
                size: QUERY_BYTES,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("asynchronous GPU timing readback"),
                size: QUERY_BYTES,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            pending: None,
        });
        Self {
            slots,
            active: None,
            mask: AtomicU32::new(0),
            next_frame: 0,
            period_ns: f64::from(queue.get_timestamp_period()),
            stats: GpuProfileStats {
                supported: true,
                enabled: true,
                ..Default::default()
            },
        }
    }

    pub(crate) fn poll(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> GpuProfileStats {
        // Poll never waits for GPU completion. Mapping callbacks can also be
        // delivered by subsequent queue submissions on native backends.
        let _ = device.poll(wgpu::PollType::Poll);
        for slot in &mut self.slots {
            let Some(mut pending) = slot.pending.take() else {
                continue;
            };
            let status = pending.status.load(Ordering::Acquire);
            if status == 0 {
                slot.pending = Some(pending);
                continue;
            }
            if pending.awaiting_gpu {
                // Metal can run counter resolves before a render pass's last
                // fragment sample, even in an immediately following command
                // buffer. Resolve only after the submission callback confirms
                // completion. This adds latency, never a CPU/GPU wait.
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("completed GPU timestamp readback"),
                });
                encoder.resolve_query_set(&slot.queries, 0..QUERY_COUNT, &slot.resolve, 0);
                encoder.copy_buffer_to_buffer(&slot.resolve, 0, &slot.readback, 0, QUERY_BYTES);
                queue.submit(Some(encoder.finish()));
                pending.status.store(0, Ordering::Release);
                let callback_status = pending.status.clone();
                slot.readback
                    .slice(..)
                    .map_async(wgpu::MapMode::Read, move |result| {
                        callback_status
                            .store(if result.is_ok() { 1 } else { 2 }, Ordering::Release);
                    });
                pending.awaiting_gpu = false;
                slot.pending = Some(pending);
                continue;
            }
            if status == 1 {
                let data = slot.readback.slice(..).get_mapped_range();
                let values: Vec<_> = data
                    .chunks_exact(8)
                    .map(|bytes| u64::from_ne_bytes(bytes.try_into().unwrap()))
                    .collect();
                if let Some(timings) =
                    decode(&values, pending.mask, self.period_ns, pending.frame_id)
                {
                    self.stats.completed += 1;
                    if self
                        .stats
                        .latest
                        .is_none_or(|old| old.frame_id < timings.frame_id)
                    {
                        self.stats.latest = Some(timings);
                    }
                } else {
                    self.stats.failed += 1;
                }
                drop(data);
            } else {
                self.stats.failed += 1;
            }
            slot.readback.unmap();
        }
        self.stats.in_flight = self
            .slots
            .iter()
            .filter(|slot| slot.pending.is_some())
            .count();
        self.stats
    }

    pub(crate) fn begin_frame(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.poll(device, queue);
        self.next_frame += 1;
        self.mask.store(0, Ordering::Relaxed);
        self.active = self.slots.iter().position(|slot| slot.pending.is_none());
        if self.active.is_none() {
            self.stats.dropped += 1;
        }
    }

    pub(crate) fn timestamps(&self, pass: Pass) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        let slot = &self.slots[self.active?];
        let index = pass as u32;
        self.mask.fetch_or(1 << index, Ordering::Relaxed);
        Some(wgpu::RenderPassTimestampWrites {
            query_set: &slot.queries,
            beginning_of_pass_write_index: Some(index * 2),
            end_of_pass_write_index: Some(index * 2 + 1),
        })
    }

    pub(crate) fn after_submit(&mut self, queue: &wgpu::Queue) {
        let Some(index) = self.active.take() else {
            return;
        };
        let slot = &mut self.slots[index];
        let status = Arc::new(AtomicU8::new(0));
        let callback_status = status.clone();
        queue.on_submitted_work_done(move || callback_status.store(1, Ordering::Release));
        slot.pending = Some(Pending {
            status,
            awaiting_gpu: true,
            frame_id: self.next_frame,
            mask: self.mask.load(Ordering::Relaxed),
        });
        self.stats.submitted += 1;
        self.stats.in_flight += 1;
    }
}

fn decode(values: &[u64], mask: u32, period_ns: f64, frame_id: u64) -> Option<GpuFrameTimings> {
    if values.len() != QUERY_COUNT as usize
        || mask == 0
        || !period_ns.is_finite()
        || period_ns <= 0.
    {
        return None;
    }
    let mut passes = [0.; 7];
    let mut raw_pass_ms = [0.; 7];
    let mut first = u64::MAX;
    let mut last = 0;
    for (index, duration) in passes.iter_mut().enumerate() {
        if mask & (1 << index) == 0 {
            continue;
        }
        let (start, end) = (values[index * 2], values[index * 2 + 1]);
        if start == 0 || end < start {
            return None;
        }
        raw_pass_ms[index] = (end - start) as f64 * period_ns / 1_000_000.;
        *duration = end.saturating_sub(start.max(last)) as f64 * period_ns / 1_000_000.;
        first = first.min(start);
        last = last.max(end);
    }
    let total_ms = passes.iter().sum();
    let raw_pass_sum_ms = raw_pass_ms.iter().sum::<f64>();
    Some(GpuFrameTimings {
        frame_id,
        shadow_ms: passes[0],
        gbuffer_ms: passes[1],
        lighting_ms: passes[2],
        transparency_ms: passes[3] + passes[4],
        particles_ms: passes[5],
        ui_ms: passes[6],
        total_ms,
        frame_span_ms: (last - first) as f64 * period_ns / 1_000_000.,
        raw_pass_ms,
        raw_pass_sum_ms,
        overlap_ms: (raw_pass_sum_ms - total_ms).max(0.),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timestamps_use_device_period_and_ignore_absent_passes_and_stale_slots() {
        let values = [
            100, 300, 400, 800, 900, 1500, 2000, 2200, 2250, 2300, 99999, 0, 88888, 0,
        ];
        let frame = decode(&values, 0b0011111, 1000., 42).unwrap();
        assert_eq!(frame.frame_id, 42);
        assert_eq!(frame.shadow_ms, 0.2);
        assert_eq!(frame.gbuffer_ms, 0.4);
        assert_eq!(frame.lighting_ms, 0.6);
        assert_eq!(frame.transparency_ms, 0.25);
        assert_eq!(frame.particles_ms, 0.);
        assert_eq!(frame.ui_ms, 0.);
        assert!((frame.total_ms - 1.45).abs() < 1e-10);
        assert_eq!(frame.frame_span_ms, 2.2);
        assert!(decode(&values, 0b1111111, 1., 1).is_none());
    }

    #[test]
    fn overlapping_tile_stages_are_retained_raw_but_not_double_counted() {
        let values = [100, 400, 200, 500, 300, 800, 0, 0, 0, 0, 0, 0, 0, 0];
        let frame = decode(&values, 0b111, 1000., 1).unwrap();
        assert_eq!(frame.raw_pass_ms[..3], [0.3, 0.3, 0.5]);
        assert_eq!(frame.shadow_ms, 0.3);
        assert_eq!(frame.gbuffer_ms, 0.1);
        assert_eq!(frame.lighting_ms, 0.3);
        assert!((frame.total_ms - 0.7).abs() < 1e-10);
        assert!((frame.raw_pass_sum_ms - 1.1).abs() < 1e-10);
        assert!((frame.overlap_ms - 0.4).abs() < 1e-10);
        assert_eq!(frame.frame_span_ms, 0.7);
    }
}
