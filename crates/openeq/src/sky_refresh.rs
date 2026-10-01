//! Background sky sampling keyed by visit and raw server minute.
use crate::loading::Job;
use openeq_assets::environment::{SkyAssets, load_sky};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub zone: String,
    pub generation: u64,
    pub hour: u8,
    pub minute: u8,
}

#[derive(Default)]
pub struct SkyRefresh {
    applied: Option<Stamp>,
    pending: Option<(Stamp, Job<SkyAssets>, bool)>,
}

impl SkyRefresh {
    /// Invalidate a departing visit without starting overlapping loader work.
    /// The retained worker can finish, but its output is always discarded even
    /// if a later session happens to request the same zone/minute stamp.
    pub fn invalidate(&mut self) {
        self.applied = None;
        if let Some((_, _, fresh)) = &mut self.pending {
            *fresh = false;
        }
    }

    /// Poll without waiting or reading files on the event loop. A stale worker
    /// may finish, but its result never becomes the current zone's sky. Retain
    /// at most one active job when minutes advance faster than loading finishes.
    pub fn poll(&mut self, desired: Stamp, directory: &Path) -> Option<Result<SkyAssets, String>> {
        self.poll_with(desired.clone(), || {
            let directory = directory.to_owned();
            Job::start(move |report| {
                report.stage("Updating sky colors", None)?;
                let fraction = (f32::from(desired.hour) + f32::from(desired.minute) / 60.) / 24.;
                let sky = load_sky(&directory, &desired.zone, fraction)?;
                report.stage("Sky colors ready", None)?;
                Ok(sky)
            })
        })
    }

    fn poll_with(
        &mut self,
        desired: Stamp,
        start: impl FnOnce() -> Job<SkyAssets>,
    ) -> Option<Result<SkyAssets, String>> {
        if let Some((stamp, job, fresh)) = &mut self.pending
            && let Some(result) = job.poll()
        {
            let current = *fresh && *stamp == desired;
            self.pending = None;
            if current {
                // Failure is terminal for this stamp too: don't retry a missing
                // authored pattern or a failed worker every rendered frame.
                self.applied = Some(desired);
                return Some(result);
            }
        }
        if self.applied.as_ref() != Some(&desired) && self.pending.is_none() {
            self.pending = Some((desired, start(), true));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openeq_assets::{environment::SkyColorMapLayout, texture::Texture};
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };

    fn stamp(minute: u8, generation: u64) -> Stamp {
        Stamp {
            zone: "poknowledge".into(),
            generation,
            hour: 6,
            minute,
        }
    }
    fn sky(name: &str) -> SkyAssets {
        SkyAssets {
            weather: name.into(),
            color_map: Texture {
                name: name.into(),
                width: 1,
                height: 1,
                rgba: vec![0; 4],
            },
            color_map_layout: SkyColorMapLayout::FullTexture,
            color_map_provenance: None,
            cloud_texture: None,
            cloud_color_map: None,
            cloud_color_map_layout: SkyColorMapLayout::FullTexture,
            cloud_color_map_provenance: None,
            cloud_velocity: 0.,
        }
    }
    fn finish(refresh: &mut SkyRefresh, stamp: Stamp) -> Result<SkyAssets, String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(result) = refresh.poll_with(stamp.clone(), || panic!("unexpected restart"))
            {
                return result;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
    }

    #[test]
    fn pending_work_is_nonblocking_and_stale_minutes_never_apply() {
        let mut refresh = SkyRefresh::default();
        let (release, waiting) = mpsc::channel();
        assert!(
            refresh
                .poll_with(stamp(0, 1), || Job::start(move |_| {
                    waiting.recv()?;
                    Ok(sky("old"))
                }))
                .is_none()
        );
        assert!(
            refresh
                .poll_with(stamp(1, 1), || panic!("overlapping job"))
                .is_none()
        );
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while refresh
            .pending
            .as_ref()
            .is_some_and(|(pending, _, _)| *pending == stamp(0, 1))
        {
            assert!(
                refresh
                    .poll_with(stamp(1, 1), || Job::start(|_| Ok(sky("new"))))
                    .is_none()
            );
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(finish(&mut refresh, stamp(1, 1)).unwrap().weather, "new");
        assert!(
            refresh
                .poll_with(stamp(1, 1), || panic!("already applied"))
                .is_none()
        );
    }

    #[test]
    fn failure_is_not_retried_each_frame_and_a_new_visit_gets_new_work() {
        let mut refresh = SkyRefresh::default();
        refresh.poll_with(stamp(2, 1), || Job::start(|_| anyhow::bail!("missing sky")));
        assert!(
            finish(&mut refresh, stamp(2, 1))
                .unwrap_err()
                .contains("missing sky")
        );
        assert!(
            refresh
                .poll_with(stamp(2, 1), || panic!("failed stamp restarted"))
                .is_none()
        );
        refresh.poll_with(stamp(2, 2), || Job::start(|_| Ok(sky("new visit"))));
        assert_eq!(
            finish(&mut refresh, stamp(2, 2)).unwrap().weather,
            "new visit"
        );
    }

    #[test]
    fn changing_visit_while_loading_discards_old_sky_even_at_same_minute() {
        let mut refresh = SkyRefresh::default();
        let (release, waiting) = mpsc::channel();
        refresh.poll_with(stamp(2, 1), || {
            Job::start(move |_| {
                waiting.recv()?;
                Ok(sky("old visit"))
            })
        });
        assert!(
            refresh
                .poll_with(stamp(2, 2), || panic!("overlapping job"))
                .is_none()
        );
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while refresh
            .pending
            .as_ref()
            .is_some_and(|(pending, _, _)| pending.generation == 1)
        {
            assert!(
                refresh
                    .poll_with(stamp(2, 2), || Job::start(|_| Ok(sky("new visit"))))
                    .is_none()
            );
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(
            finish(&mut refresh, stamp(2, 2)).unwrap().weather,
            "new visit"
        );
    }
    #[test]
    fn visit_reset_retains_one_worker_and_discards_even_an_identical_stamp() {
        let mut refresh = SkyRefresh::default();
        let (release, waiting) = mpsc::channel();
        refresh.poll_with(stamp(2, 1), || {
            Job::start(move |_| {
                waiting.recv()?;
                Ok(sky("departed visit"))
            })
        });
        for _ in 0..3 {
            refresh.invalidate();
            assert!(
                refresh
                    .poll_with(stamp(2, 1), || panic!("reset overlapped work"))
                    .is_none()
            );
        }
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while refresh.pending.as_ref().is_some_and(|(_, _, fresh)| !fresh) {
            assert!(
                refresh
                    .poll_with(stamp(2, 1), || Job::start(|_| Ok(sky("current visit"))))
                    .is_none()
            );
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(
            finish(&mut refresh, stamp(2, 1)).unwrap().weather,
            "current visit"
        );
    }
}
