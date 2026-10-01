//! Opt-in windowed frame measurements. CPU wall time includes any driver
//! backpressure; GPU passes run asynchronously and must not be added to it.
use std::{collections::BTreeMap, time::Instant};

use openeq_render::Renderer;

type Samples = BTreeMap<&'static str, Vec<f64>>;

pub struct FrameSample {
    start: Instant,
    mark: Instant,
    stages: Vec<(&'static str, f64)>,
}

impl FrameSample {
    pub fn mark(sample: &mut Option<Self>, stage: &'static str) {
        if let Some(sample) = sample {
            let now = Instant::now();
            sample
                .stages
                .push((stage, (now - sample.mark).as_secs_f64() * 1000.));
            sample.mark = now;
        }
    }
}

pub struct FrameProfiler {
    enabled: bool,
    last_start: Option<Instant>,
    report_start: Instant,
    cpu: Samples,
    gpu: Samples,
    last_gpu: Option<u64>,
}

impl Default for FrameProfiler {
    fn default() -> Self {
        Self {
            enabled: std::env::var("OPENEQ_PROFILE").is_ok_and(|v| v == "1"),
            last_start: None,
            report_start: Instant::now(),
            cpu: Samples::new(),
            gpu: Samples::new(),
            last_gpu: None,
        }
    }
}

impl FrameProfiler {
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn begin(&self) -> Option<FrameSample> {
        self.enabled.then(|| {
            let now = Instant::now();
            FrameSample {
                start: now,
                mark: now,
                stages: Vec::with_capacity(10),
            }
        })
    }

    /// Loading pauses do not belong in steady-state frame statistics.
    pub fn reset(&mut self) {
        self.last_start = None;
        self.report_start = Instant::now();
        self.cpu.clear();
        self.gpu.clear();
    }

    pub fn finish(
        &mut self,
        sample: Option<FrameSample>,
        renderer: &mut Renderer,
        size: (u32, u32),
    ) {
        let Some(sample) = sample else { return };
        self.cpu
            .entry("cpu_frame")
            .or_default()
            .push(sample.start.elapsed().as_secs_f64() * 1000.);
        if let Some(last) = self.last_start.replace(sample.start) {
            self.cpu
                .entry("frame_interval")
                .or_default()
                .push((sample.start - last).as_secs_f64() * 1000.);
        }
        for (name, ms) in sample.stages {
            self.cpu.entry(name).or_default().push(ms);
        }
        let stats = renderer.profiling_stats();
        if let Some(gpu) = stats.latest
            && self.last_gpu != Some(gpu.frame_id)
        {
            self.last_gpu = Some(gpu.frame_id);
            for (name, ms) in [
                ("shadow", gpu.shadow_ms),
                ("gbuffer", gpu.gbuffer_ms),
                ("lighting", gpu.lighting_ms),
                ("transparency", gpu.transparency_ms),
                ("additive", gpu.additive_ms),
                ("particles", gpu.particles_ms),
                ("ui", gpu.ui_ms),
                ("gpu_frame", gpu.total_ms),
                ("frame_span", gpu.frame_span_ms),
            ] {
                self.gpu.entry(name).or_default().push(ms);
            }
        }
        // Bound memory even when profiling an extremely fast empty scene.
        if self.report_start.elapsed().as_secs_f32() >= 3.
            || self.cpu.get("cpu_frame").is_some_and(|s| s.len() >= 4096)
        {
            tracing::info!(
                width = size.0,
                height = size.1,
                frames = self.cpu["cpu_frame"].len(),
                "PROFILE CPU milliseconds median/p95: {}",
                summarize(&mut self.cpu)
            );
            tracing::info!(
                supported = stats.supported,
                completed = stats.completed,
                dropped = stats.dropped,
                failed = stats.failed,
                "PROFILE GPU completion attribution milliseconds median/p95: {}",
                summarize(&mut self.gpu)
            );
            self.cpu.clear();
            self.gpu.clear();
            self.report_start = Instant::now();
        }
    }
}

fn summarize(samples: &mut Samples) -> String {
    samples
        .iter_mut()
        .filter(|(_, values)| !values.is_empty())
        .map(|(name, values)| {
            values.sort_by(f64::total_cmp);
            let percentile =
                |p: f64| values[((values.len() as f64 * p).ceil() as usize).saturating_sub(1)];
            format!("{name}={:.2}/{:.2}", percentile(0.5), percentile(0.95))
        })
        .collect::<Vec<_>>()
        .join(" ")
}
