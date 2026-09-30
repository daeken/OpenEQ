use super::{
    decode::{self, Effect},
    schedule::{Channel, Scheduler, Voice},
    settings::{self, Levels},
};
use openeq_assets::audio::{AudioAssetLocation, AudioCatalog, ZoneAudio};
use rodio::{Player, Source};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

const CACHE_BYTES: usize = 64 * 1024 * 1024;
const EFFECT_JOBS: usize = 2; // Reserve two decoders for music crossfades.

#[derive(Clone)]
struct Desired {
    zone: Option<(u64, String)>,
    position: [f32; 3],
    hour: u8,
    levels: Levels,
    stop: bool,
}
#[derive(Default)]
struct Status {
    output: String,
    zone: Option<(u64, String)>,
    music: Option<String>,
    effects: usize,
    pending: usize,
    cache_bytes: usize,
}
/// Device ownership, metadata reads and decoding stay off the game thread.
/// The shared control slot coalesces listener changes instead of queueing them.
pub struct AudioService {
    desired: Arc<Mutex<Desired>>,
    status: Arc<Mutex<Status>>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl AudioService {
    pub fn new(dir: PathBuf, enabled: bool) -> Self {
        let mut path = settings::settings_path();
        let levels = path
            .as_ref()
            .map_or_else(|| Ok(Levels::default()), |path| settings::load(path))
            .unwrap_or_else(|error| {
                tracing::warn!(%error,"audio settings unavailable; preserving saved file");
                path = None;
                Levels::default()
            });
        Self::start(dir, enabled, path, levels)
    }
    fn start(dir: PathBuf, enabled: bool, path: Option<PathBuf>, levels: Levels) -> Self {
        let desired = Arc::new(Mutex::new(Desired {
            zone: None,
            position: [0.; 3],
            hour: 12,
            levels,
            stop: false,
        }));
        let status = Arc::new(Mutex::new(Status {
            output: "starting".into(),
            ..Default::default()
        }));
        let control = Arc::clone(&desired);
        let report = Arc::clone(&status);
        let worker = std::thread::Builder::new()
            .name("openeq-audio".into())
            .spawn(move || run(dir, enabled, path, levels, control, report))
            .ok();
        if worker.is_none() {
            status.lock().unwrap().output = "unavailable".into();
        }
        Self {
            desired,
            status,
            worker,
        }
    }
    /// None stops all zone voices, including during a same-zone re-entry.
    pub fn update(&self, zone: Option<(u64, &str)>, position: [f32; 3], hour: u8) {
        let mut desired = self.desired.lock().unwrap();
        if desired
            .zone
            .as_ref()
            .map(|(generation, name)| (*generation, name.as_str()))
            != zone
        {
            desired.zone = zone.map(|(generation, name)| (generation, name.to_string()));
        }
        desired.position = position;
        desired.hour = hour;
    }
    pub fn command(&self, text: &str) -> Option<String> {
        let response = self.desired.lock().unwrap().levels.command(text)?;
        Some(format!(
            "{response} Output: {}.",
            self.status.lock().unwrap().output
        ))
    }
}
impl Drop for AudioService {
    fn drop(&mut self) {
        self.desired.lock().unwrap().stop = true;
        // Shutdown waits for the service, never for a music worker blocked on
        // playback: dropping its source disconnects the bounded sample queue.
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct Output {
    #[cfg(feature = "audio-playback")]
    _device: Option<rodio::MixerDeviceSink>,
    mixer: Option<rodio::mixer::Mixer>,
}
impl Output {
    fn open(enabled: bool) -> (Self, String) {
        #[cfg(feature = "audio-playback")]
        if enabled {
            match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(mut device) => {
                    device.log_on_drop(false);
                    let (mixer, source) = rodio::mixer::mixer(
                        device.config().channel_count(),
                        device.config().sample_rate(),
                    );
                    // Keep the subordinate mix alive through silence and limit
                    // the combined output, rather than clipping summed voices.
                    mixer.add(rodio::source::Zero::new(
                        device.config().channel_count(),
                        device.config().sample_rate(),
                    ));
                    device.mixer().add(source.limit(Default::default()));
                    return (
                        Self {
                            _device: Some(device),
                            mixer: Some(mixer),
                        },
                        "on".into(),
                    );
                }
                Err(error) => tracing::warn!(%error,"no audio device; continuing silently"),
            }
        }
        (
            Self {
                #[cfg(feature = "audio-playback")]
                _device: None,
                mixer: None,
            },
            if enabled { "unavailable" } else { "disabled" }.into(),
        )
    }
}
struct Playing {
    player: Player,
    voice: Voice,
    started: Instant,
    gain: f32,
    music_failure: Option<Arc<AtomicBool>>,
}
impl Playing {
    fn update_gain(&mut self, levels: Levels, now: Instant) {
        let fade = if self.voice.fade_in.is_zero() {
            1.
        } else {
            (now.duration_since(self.started).as_secs_f32() / self.voice.fade_in.as_secs_f32())
                .min(1.)
        };
        let target = self.voice.gain * levels.gain(self.voice.channel == Channel::Music) * fade;
        self.gain = if levels.muted || levels.master == 0. {
            0.
        } else {
            self.gain + (target - self.gain) * 0.25
        };
        self.player.set_volume(self.gain);
    }
}
struct Cached {
    effect: Effect,
    used: u64,
}
struct Completion {
    epoch: u64,
    token: u64,
    file: String,
    result: anyhow::Result<Effect>,
}
fn run(
    dir: PathBuf,
    enabled: bool,
    path: Option<PathBuf>,
    loaded_levels: Levels,
    control: Arc<Mutex<Desired>>,
    status: Arc<Mutex<Status>>,
) {
    let (output, output_status) = Output::open(enabled);
    status.lock().unwrap().output = output_status;
    let catalog = match AudioCatalog::load(&dir) {
        Ok(catalog) => Arc::new(catalog),
        Err(error) => {
            tracing::warn!(%error,"audio assets unavailable");
            status.lock().unwrap().output = "assets unavailable".into();
            return;
        }
    };
    let mut scheduler = Scheduler::default();
    let mut zone = None;
    let mut epoch = 0u64;
    let mut players: BTreeMap<u64, Playing> = BTreeMap::new();
    let mut outgoing: Option<(Playing, Instant)> = None;
    let mut fading_effects: BTreeMap<u64, (Playing, Instant)> = BTreeMap::new();
    let mut cache: BTreeMap<String, Cached> = BTreeMap::new();
    let mut cache_size = 0;
    let mut stamp = 0;
    let mut pending: BTreeSet<(u64, String)> = BTreeSet::new();
    let (tx, rx) = mpsc::sync_channel::<Completion>(EFFECT_JOBS);
    let start = Instant::now();
    let mut saved = loaded_levels;
    let mut next_save = Instant::now();
    loop {
        let desired = control.lock().unwrap().clone();
        let now = Instant::now();
        let elapsed = now.duration_since(start);
        if desired.levels != saved && (now >= next_save || desired.stop) {
            if let Some(path) = &path {
                match settings::save(path, desired.levels) {
                    Ok(()) => saved = desired.levels,
                    Err(error) => {
                        tracing::warn!(%error,"could not save audio levels");
                        next_save = now + Duration::from_secs(5);
                    }
                }
            } else {
                saved = desired.levels;
            }
        }
        if desired.stop {
            break;
        }
        if desired.zone != zone {
            players.clear();
            outgoing = None;
            fading_effects.clear();
            epoch = epoch.wrapping_add(1);
            zone = desired.zone.clone();
            let metadata = zone.as_ref().map(|(_, name)| catalog.load_zone(name));
            match metadata {
                Some(Ok(zone)) => {
                    if !zone.diagnostics.entries.is_empty() || zone.diagnostics.suppressed > 0 {
                        tracing::info!(zone=%zone.zone, diagnostics=zone.diagnostics.entries.len(), suppressed=zone.diagnostics.suppressed,"authored audio metadata diagnostics");
                    }
                    scheduler.set_zone(&zone);
                }
                Some(Err(error)) => {
                    tracing::warn!(%error,"zone audio unavailable");
                    scheduler.set_zone(&ZoneAudio::default());
                }
                None => scheduler.set_zone(&ZoneAudio::default()),
            }
            // A load completed after the game requested another destination.
            if control.lock().unwrap().zone != zone {
                continue;
            }
        }
        let finished: Vec<_> = players
            .iter()
            .filter_map(|(token, playing)| playing.player.empty().then_some(*token))
            .collect();
        for token in finished {
            let failed = players.remove(&token).is_some_and(|playing| {
                playing
                    .music_failure
                    .is_some_and(|flag| flag.load(Ordering::Acquire))
            });
            scheduler.finished(token, elapsed, failed);
        }
        let voices = scheduler.update(
            elapsed,
            desired.position,
            desired.hour,
            desired.levels.environment_enabled,
        );
        let wanted: BTreeSet<_> = voices.iter().map(|voice| voice.token).collect();
        {
            let mut status = status.lock().unwrap();
            status.zone = zone.clone();
            status.music = voices
                .iter()
                .find(|v| v.channel == Channel::Music)
                .map(|v| v.file.clone());
            status.effects = voices
                .iter()
                .filter(|v| v.channel == Channel::Ambience)
                .count();
            status.pending = pending.len();
            status.cache_bytes = cache_size;
        }
        let expired: Vec<_> = players
            .keys()
            .filter(|token| !wanted.contains(token))
            .copied()
            .collect();
        for token in expired {
            let playing = players.remove(&token).unwrap();
            if playing.voice.channel == Channel::Music && !playing.voice.fade_out.is_zero() {
                outgoing = Some((playing, now)); // One outgoing track at most.
            } else if !playing.voice.fade_out.is_zero() {
                fading_effects.insert(token, (playing, now));
            }
        }
        fading_effects.retain(|_, (playing, began)| {
            let ratio = 1.
                - now.duration_since(*began).as_secs_f32() / playing.voice.fade_out.as_secs_f32();
            if ratio <= 0. || playing.player.empty() {
                return false;
            }
            playing.player.set_volume(
                playing
                    .gain
                    .min(playing.voice.gain * desired.levels.gain(false))
                    * ratio,
            );
            true
        });
        if let Some((playing, began)) = &outgoing {
            let ratio = 1.
                - now.duration_since(*began).as_secs_f32() / playing.voice.fade_out.as_secs_f32();
            if ratio <= 0. || playing.player.empty() {
                outgoing = None;
            } else {
                playing.player.set_volume(
                    playing
                        .gain
                        .min(playing.voice.gain * desired.levels.gain(true))
                        * ratio,
                );
            }
        }
        let mut failed_tokens = BTreeSet::new();
        while let Ok(completion) = rx.try_recv() {
            pending.remove(&(completion.epoch, completion.file.clone()));
            if completion.epoch != epoch || !wanted.contains(&completion.token) {
                continue;
            }
            match completion.result {
                Ok(effect) => {
                    while cache_size + effect.bytes > CACHE_BYTES {
                        let victim = cache
                            .iter()
                            .filter(|(file, _)| {
                                !players.values().any(|p| p.voice.file == **file)
                                    && !fading_effects.values().any(|(p, _)| p.voice.file == **file)
                            })
                            .min_by_key(|(_, entry)| entry.used)
                            .map(|(file, _)| file.clone());
                        let Some(victim) = victim else {
                            break;
                        };
                        cache_size -= cache.remove(&victim).unwrap().effect.bytes;
                    }
                    if cache_size + effect.bytes <= CACHE_BYTES {
                        if let Some(old) = cache.remove(&completion.file) {
                            cache_size -= old.effect.bytes;
                        }
                        cache_size += effect.bytes;
                        cache.insert(
                            completion.file,
                            Cached {
                                effect,
                                used: stamp,
                            },
                        );
                    } else {
                        scheduler.finished(completion.token, elapsed, true);
                        failed_tokens.insert(completion.token);
                    }
                }
                Err(error) => {
                    tracing::warn!(file=%completion.file,%error,"sound unavailable");
                    scheduler.finished(completion.token, elapsed, true);
                    failed_tokens.insert(completion.token);
                }
            }
        }
        for voice in voices {
            if failed_tokens.contains(&voice.token) {
                continue;
            }
            if let Some(playing) = players.get_mut(&voice.token) {
                playing.voice.gain = voice.gain;
                playing.update_gain(desired.levels, now);
                continue;
            }
            let Some(mixer) = &output.mixer else {
                continue;
            };
            let mut music_failure = None;
            let source: Box<dyn Source<Item = f32> + Send> = if voice.channel == Channel::Music {
                // Original MP3 music is loose; archived streaming can follow
                // once an incremental PFS reader exists.
                let result = if let Some(sequence) = voice.sequence {
                    (|| {
                        let bytes = catalog
                            .read(&voice.file)?
                            .ok_or_else(|| anyhow::anyhow!("XMI file unavailable"))?;
                        super::xmi::stream::stream(bytes, sequence)
                    })()
                } else {
                    match catalog.asset(&voice.file).map(|asset| &asset.location) {
                        Some(AudioAssetLocation::Loose(path)) => decode::stream_music(path),
                        _ => Err(anyhow::anyhow!("streaming music file unavailable")),
                    }
                };
                match result {
                    Ok(source) => {
                        music_failure = Some(source.failure());
                        Box::new(source)
                    }
                    Err(error) => {
                        if error.downcast_ref::<super::xmi::stream::Busy>().is_some() {
                            // A cancelled synth can still be disposing on its
                            // owning worker. Retry this occurrence, do not mark
                            // valid music unavailable for the entire zone.
                            continue;
                        }
                        tracing::warn!(file=%voice.file,sequence=?voice.sequence,%error,"music unavailable");
                        scheduler.finished(voice.token, elapsed, true);
                        continue;
                    }
                }
            } else if let Some(cached) = cache.get_mut(&voice.file) {
                stamp += 1;
                cached.used = stamp;
                if voice.continuous {
                    Box::new(cached.effect.samples.clone().repeat_infinite())
                } else {
                    Box::new(cached.effect.samples.clone())
                }
            } else {
                if pending.len() < EFFECT_JOBS && !pending.contains(&(epoch, voice.file.clone())) {
                    let catalog = Arc::clone(&catalog);
                    let sender = tx.clone();
                    let file = voice.file.clone();
                    let token = voice.token;
                    pending.insert((epoch, file.clone()));
                    let spawn = std::thread::Builder::new()
                        .name("openeq-effect-decode".into())
                        .spawn(move || {
                            let result = (|| -> anyhow::Result<Effect> {
                                let bytes = catalog
                                    .read(&file)?
                                    .ok_or_else(|| anyhow::anyhow!("sound file not found"))?;
                                decode::decode_effect(bytes)
                            })();
                            let _ = sender.send(Completion {
                                epoch,
                                token,
                                file,
                                result,
                            });
                        });
                    if spawn.is_err() {
                        pending.remove(&(epoch, voice.file.clone()));
                        scheduler.finished(voice.token, elapsed, true);
                    }
                }
                continue;
            };
            let fresh = control.lock().unwrap().clone();
            if fresh.stop || fresh.zone != zone {
                // File parsing/decoder construction may finish after zoning.
                // Drop the bounded source before it can reach the mixer.
                continue;
            }
            let player = Player::connect_new(mixer);
            if voice.channel == Channel::Ambience {
                while players
                    .values()
                    .filter(|p| p.voice.channel == Channel::Ambience)
                    .count()
                    + fading_effects.len()
                    >= super::schedule::MAX_EFFECT_VOICES
                {
                    let oldest = fading_effects
                        .iter()
                        .min_by_key(|(_, (_, began))| *began)
                        .map(|(token, _)| *token);
                    if let Some(token) = oldest {
                        fading_effects.remove(&token);
                    } else {
                        break;
                    }
                }
            }
            player.set_volume(0.);
            player.append(source);
            let mut playing = Playing {
                player,
                voice,
                started: now,
                gain: 0.,
                music_failure,
            };
            playing.update_gain(fresh.levels, now);
            players.insert(playing.voice.token, playing);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires a default audio device; renders only digital silence"]
    #[cfg(feature = "audio-playback")]
    fn device_opens_and_closes_with_only_silence() {
        let (output, status) = Output::open(true);
        assert_eq!(status, "on");
        assert!(output.mixer.is_some());
        std::thread::sleep(Duration::from_millis(30));
        drop(output);
    }
    #[test]
    fn offline_mixer_resamples_stereo_and_obeys_mute_and_stop() {
        use std::num::NonZero;
        let channels = NonZero::new(2).unwrap();
        let rate = NonZero::new(48000).unwrap();
        let (mixer, source) = rodio::mixer::mixer(channels, rate);
        mixer.add(rodio::source::Zero::new(channels, rate));
        let mut source = source.limit(Default::default());
        let player = Player::connect_new(&mixer);
        player.set_volume(0.5);
        player.append(
            rodio::buffer::SamplesBuffer::new(
                NonZero::new(1).unwrap(),
                NonZero::new(22000).unwrap(),
                vec![0.5; 22000],
            )
            .repeat_infinite(),
        );
        let samples: Vec<_> = source.by_ref().take(9600).collect();
        assert!(samples.iter().any(|s| *s > 0.2));
        assert!(samples.iter().all(|s| s.abs() <= 0.26));
        assert!(
            samples
                .chunks_exact(2)
                .all(|frame| (frame[0] - frame[1]).abs() < 0.001)
        );
        player.set_volume(0.);
        let _ = source.by_ref().take(2400).count();
        assert!(source.by_ref().take(2400).all(|s| s.abs() < 0.001));
        drop(player);
        let _ = source.by_ref().take(2400).count();
        assert!(source.take(2400).all(|s| s == 0.));
    }
    fn wait(service: &AudioService, condition: impl Fn(&Status) -> bool) {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            if condition(&service.status.lock().unwrap()) {
                return;
            }
            assert!(Instant::now() < until, "silent audio worker timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    #[ignore = "requires original client audio metadata; never opens an output device"]
    fn original_zones_silent_service_cancels_same_zone_and_cross_zone_audio() {
        let preferences = std::env::temp_dir().join(format!(
            "openeq-audio-test-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let service = AudioService::start(
            openeq_assets::loader::default_client_dir().unwrap(),
            false,
            Some(preferences.clone()),
            Levels::default(),
        );
        // A command issued while metadata/device setup is in progress must
        // still differ from the originally loaded persistence baseline.
        service.command("/audio music 25");
        service.update(Some((1, "anguish")), [0.; 3], 12);
        wait(&service, |s| {
            s.zone.as_ref().is_some_and(|(g, _)| *g == 1)
                && s.music.as_deref() == Some("anguish.mp3")
        });
        {
            let status = service.status.lock().unwrap();
            assert_eq!(status.output, "disabled");
            assert_eq!(status.pending, 0);
            assert_eq!(status.cache_bytes, 0);
        }
        service.update(None, [0.; 3], 12);
        wait(&service, |s| {
            s.zone.is_none() && s.music.is_none() && s.effects == 0
        });
        service.update(Some((2, "poknowledge")), [-0.451, 1489.518, -124.249], 12);
        wait(&service, |s| s.music.as_deref() == Some("poknowledge.mp3"));
        service.update(Some((3, "poknowledge")), [-782.696, 890.960, -147.947], 23);
        wait(&service, |s| {
            s.zone.as_ref().is_some_and(|(g, _)| *g == 3) && s.effects > 0
        });
        service.update(None, [0.; 3], 12);
        wait(&service, |s| {
            s.zone.is_none() && s.music.is_none() && s.effects == 0
        });
        drop(service);
        assert_eq!(settings::load(&preferences).unwrap().music, 0.25);
        std::fs::remove_file(preferences).unwrap();
    }
}
