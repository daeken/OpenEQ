//! Nonblocking asset jobs. The event loop polls; it never joins a loader thread.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    pub detail: String,
    pub fraction: Option<f32>,
}

enum Event<T> {
    Progress(Progress),
    Finished(Result<T, String>),
}

/// Sending a stage also checks whether this job was superseded by another zone.
pub struct Reporter<T> {
    tx: mpsc::Sender<Event<T>>,
    cancelled: Arc<AtomicBool>,
}
impl<T> Reporter<T> {
    pub fn stage(&self, detail: impl Into<String>, fraction: Option<f32>) -> anyhow::Result<()> {
        anyhow::ensure!(!self.cancelled.load(Ordering::Relaxed), "Loading cancelled");
        self.tx
            .send(Event::Progress(Progress {
                detail: detail.into(),
                fraction,
            }))
            .map_err(|_| anyhow::anyhow!("Loading cancelled"))
    }
}

pub struct Job<T> {
    rx: Mutex<mpsc::Receiver<Event<T>>>,
    cancelled: Arc<AtomicBool>,
    pub progress: Progress,
    finished: bool,
}
impl<T: Send + 'static> Job<T> {
    pub fn start(work: impl FnOnce(Reporter<T>) -> anyhow::Result<T> + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let reporter = Reporter {
            tx: tx.clone(),
            cancelled: cancelled.clone(),
        };
        let spawn = std::thread::Builder::new()
            .name("zone-loader".into())
            .spawn(move || {
                // A corrupt asset must not turn into an endless loading spinner.
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(reporter)))
                        .map_err(|_| "The asset loader stopped unexpectedly.".to_owned())
                        .and_then(|result| result.map_err(|error| format!("{error:#}")));
                let _ = tx.send(Event::Finished(result));
            });
        let mut job = Self {
            rx: Mutex::new(rx),
            cancelled,
            progress: Progress {
                detail: "Preparing your journey…".into(),
                fraction: None,
            },
            finished: false,
        };
        if let Err(error) = spawn {
            job.progress.detail = format!("Unable to start loading: {error}");
        }
        job
    }

    /// Returns a result once, without waiting for any worker activity.
    pub fn poll(&mut self) -> Option<Result<T, String>> {
        if self.finished {
            return None;
        }
        let rx = self.rx.get_mut().unwrap();
        loop {
            match rx.try_recv() {
                Ok(Event::Progress(progress)) => self.progress = progress,
                Ok(Event::Finished(result)) => {
                    self.finished = true;
                    return Some(result);
                }
                Err(mpsc::TryRecvError::Empty) => return None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.finished = true;
                    return Some(Err(
                        "The asset loader stopped before loading finished.".into()
                    ));
                }
            }
        }
    }
}
impl<T> Drop for Job<T> {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

/// Includes the server transition serial: re-entering the same zone is a new load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Destination {
    pub zone: String,
    pub generation: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn finish<T: Send + 'static>(job: &mut Job<T>) -> Result<T, String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(result) = job.poll() {
                return result;
            }
            assert!(Instant::now() < deadline, "worker timed out");
            std::thread::yield_now();
        }
    }

    #[test]
    fn polling_and_drop_do_not_wait_for_work() {
        let (release, wait) = mpsc::channel();
        let (started, ready) = mpsc::channel();
        let (cancelled, observed) = mpsc::channel();
        let mut job = Job::start(move |report| {
            report.stage("Reading textures", Some(0.4))?;
            started.send(()).unwrap();
            wait.recv().unwrap();
            cancelled
                .send(report.stage("Done", Some(1.)).is_err())
                .unwrap();
            Ok(())
        });
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(job.poll().is_none());
        assert_eq!(job.progress.detail, "Reading textures");
        drop(job); // Would deadlock if either poll or drop joined the worker.
        release.send(()).unwrap();
        assert!(observed.recv_timeout(Duration::from_secs(5)).unwrap());
    }

    #[test]
    fn errors_and_panics_are_terminal_results() {
        let mut job: Job<()> = Job::start(|_| anyhow::bail!("Missing zone archive"));
        assert!(
            finish(&mut job)
                .unwrap_err()
                .contains("Missing zone archive")
        );
        assert!(job.poll().is_none());
        let mut job: Job<()> = Job::start(|_| panic!("corrupt model"));
        assert!(finish(&mut job).unwrap_err().contains("unexpectedly"));
    }

    #[test]
    fn a_new_visit_to_the_same_zone_is_a_different_destination() {
        assert_ne!(
            Destination {
                zone: "gfaydark".into(),
                generation: 1
            },
            Destination {
                zone: "gfaydark".into(),
                generation: 2
            }
        );
    }
}
