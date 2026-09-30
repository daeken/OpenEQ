//! Bounded decoding. Music is decoded ahead on a worker; the output callback
//! only consumes prepared samples and never opens, reads or decompresses files.
use anyhow::{Context, Result, ensure};
use rodio::{Decoder, Source, buffer::SamplesBuffer};
use std::{
    io::Cursor,
    num::NonZero,
    path::Path,
    sync::{
        Arc,
        atomic::AtomicBool,
        mpsc::{self, Receiver, TryRecvError},
    },
    time::Duration,
};

pub const MAX_SOURCE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_EFFECT_SAMPLES: usize = 4 * 1024 * 1024;
const MAX_MUSIC_SECONDS: usize = 30 * 60;
const STREAM_FRAMES: usize = 4096;

#[derive(Clone)]
pub struct Effect {
    pub samples: SamplesBuffer,
    pub bytes: usize,
}

fn validate_format(source: &impl Source) -> Result<()> {
    ensure!(source.channels().get() <= 2, "audio must be mono or stereo");
    ensure!(
        (8000..=192000).contains(&source.sample_rate().get()),
        "unsupported audio sample rate"
    );
    Ok(())
}

pub fn decode_effect(bytes: Vec<u8>) -> Result<Effect> {
    ensure!(
        bytes.len() <= MAX_SOURCE_BYTES,
        "audio source exceeds size limit"
    );
    let source = Decoder::try_from(Cursor::new(bytes)).context("decoding effect")?;
    validate_format(&source)?;
    let channels = source.channels();
    let rate = source.sample_rate();
    let samples: Vec<_> = source.take(MAX_EFFECT_SAMPLES + 1).collect();
    ensure!(
        samples.len() <= MAX_EFFECT_SAMPLES,
        "effect exceeds decoded sample limit"
    );
    ensure!(!samples.is_empty(), "effect contains no samples");
    ensure!(
        samples.len().is_multiple_of(usize::from(channels.get())),
        "partial audio frame"
    );
    ensure!(
        samples.iter().all(|sample| sample.is_finite()),
        "nonfinite audio samples"
    );
    Ok(Effect {
        bytes: samples.len() * size_of::<f32>(),
        samples: SamplesBuffer::new(channels, rate, samples),
    })
}

/// An eight-block queue plus producer/consumer blocks uses at most 320 KiB of
/// stereo sample storage, excluding decoder internals. Dropping the
/// source disconnects the worker's send, including while a player is paused.
pub struct MusicStream {
    receiver: Receiver<Vec<f32>>,
    block: std::vec::IntoIter<f32>,
    channels: NonZero<u16>,
    rate: NonZero<u32>,
    duration: Option<Duration>,
    silence: u16,
    ended: bool,
    failure: Arc<AtomicBool>,
}

impl MusicStream {
    pub(super) fn from_queue(
        receiver: Receiver<Vec<f32>>,
        channels: NonZero<u16>,
        rate: NonZero<u32>,
        duration: Option<Duration>,
        failure: Arc<AtomicBool>,
    ) -> Self {
        Self {
            receiver,
            block: Vec::new().into_iter(),
            channels,
            rate,
            duration,
            silence: 0,
            ended: false,
            failure,
        }
    }
    pub(super) fn failure(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.failure)
    }
}

pub fn stream_music(path: &Path) -> Result<MusicStream> {
    let file = std::fs::File::open(path).context("opening music")?;
    ensure!(
        file.metadata()?.len() <= MAX_SOURCE_BYTES as u64,
        "music source exceeds size limit"
    );
    let source = Decoder::try_from(file).context("decoding music")?;
    validate_format(&source)?;
    let channels = source.channels();
    let rate = source.sample_rate();
    let duration = source.total_duration();
    let (sender, receiver) = mpsc::sync_channel(8);
    std::thread::Builder::new()
        .name("openeq-music-decode".into())
        .spawn(move || {
            let max = MAX_MUSIC_SECONDS * rate.get() as usize * usize::from(channels.get());
            let mut source = source.take(max);
            loop {
                let block: Vec<_> = source
                    .by_ref()
                    .take(STREAM_FRAMES * usize::from(channels.get()))
                    .map(|sample| {
                        if sample.is_finite() {
                            sample.clamp(-1., 1.)
                        } else {
                            0.
                        }
                    })
                    .collect();
                if block.is_empty() || sender.send(block).is_err() {
                    break;
                }
            }
        })?;
    Ok(MusicStream {
        receiver,
        block: Vec::new().into_iter(),
        channels,
        rate,
        duration,
        silence: 0,
        ended: false,
        failure: Arc::default(),
    })
}

impl Iterator for MusicStream {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.ended {
            return None;
        }
        if self.silence > 0 {
            self.silence -= 1;
            return Some(0.);
        }
        if let Some(sample) = self.block.next() {
            return Some(sample);
        }
        match self.receiver.try_recv() {
            Ok(block) => {
                self.block = block.into_iter();
                self.block.next()
            }
            Err(TryRecvError::Empty) => {
                // Fill a whole frame so an underrun cannot swap stereo channels.
                self.silence = self.channels.get() - 1;
                Some(0.)
            }
            Err(TryRecvError::Disconnected) => {
                self.ended = true;
                None
            }
        }
    }
}
impl Source for MusicStream {
    fn current_span_len(&self) -> Option<usize> {
        self.ended.then_some(0)
    }
    fn channels(&self) -> NonZero<u16> {
        self.channels
    }
    fn sample_rate(&self) -> NonZero<u32> {
        self.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        self.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wave(bits: u16, channels: u16, rate: u32) -> Vec<u8> {
        let mut out = b"RIFF".to_vec();
        let frame = match bits {
            8 => vec![192],
            16 => 16384i16.to_le_bytes().to_vec(),
            24 => vec![0, 0, 64],
            _ => unreachable!(),
        };
        let samples: Vec<_> = frame.repeat(channels as usize * 16);
        out.extend((36 + samples.len() as u32).to_le_bytes());
        out.extend(b"WAVEfmt ");
        out.extend(16u32.to_le_bytes());
        out.extend(1u16.to_le_bytes());
        out.extend(channels.to_le_bytes());
        out.extend(rate.to_le_bytes());
        out.extend((rate * u32::from(channels * bits / 8)).to_le_bytes());
        out.extend((channels * bits / 8).to_le_bytes());
        out.extend(bits.to_le_bytes());
        out.extend(b"data");
        out.extend((samples.len() as u32).to_le_bytes());
        out.extend(samples);
        out
    }
    #[test]
    fn pcm_depths_channels_and_unusual_rates_decode_silently() {
        for bits in [8, 16, 24] {
            for channels in [1, 2] {
                for rate in [8000, 22000, 44100] {
                    let effect = decode_effect(wave(bits, channels, rate)).unwrap();
                    assert_eq!(effect.samples.channels().get(), channels);
                    assert_eq!(effect.samples.sample_rate().get(), rate);
                    assert_eq!(effect.bytes, 16 * channels as usize * 4);
                    assert!(effect.samples.clone().all(|s| (s - 0.5).abs() < 0.005));
                }
            }
        }
        assert!(decode_effect(vec![0; 64]).is_err());
        assert!(decode_effect(wave(16, 3, 22000)).is_err());
    }
    #[test]
    fn underrun_preserves_stereo_alignment_and_end_is_final() {
        let (tx, rx) = mpsc::sync_channel(1);
        let mut stream = MusicStream {
            receiver: rx,
            block: Vec::new().into_iter(),
            channels: NonZero::new(2).unwrap(),
            rate: NonZero::new(22000).unwrap(),
            duration: None,
            silence: 0,
            ended: false,
            failure: Arc::default(),
        };
        assert_eq!(stream.next(), Some(0.));
        tx.send(vec![0.25, 0.75]).unwrap();
        assert_eq!(stream.next(), Some(0.));
        assert_eq!(stream.next(), Some(0.25));
        assert_eq!(stream.next(), Some(0.75));
        drop(tx);
        assert_eq!(stream.next(), None);
        assert_eq!(stream.next(), None);
        assert_eq!(stream.current_span_len(), Some(0));
    }
    #[test]
    #[ignore = "requires original client audio; decoding only, no output device"]
    fn original_archived_ambience_and_streamed_mp3_decode_silently() {
        let dir = openeq_assets::loader::default_client_dir().unwrap();
        let catalog = openeq_assets::audio::AudioCatalog::load(&dir).unwrap();
        for file in [
            "nightime_background02_lp.wav",
            "scientist_lab_lp.wav",
            "wind_lp2.wav",
        ] {
            let effect = decode_effect(catalog.read(file).unwrap().unwrap()).unwrap();
            assert!(effect.bytes > 4096);
            assert!(effect.samples.clone().any(|sample| sample.abs() > 0.01));
        }
        for file in ["poknowledge.mp3", "anguish.mp3"] {
            let stream = stream_music(&dir.join(file)).unwrap();
            let rate = stream.rate.get() as usize;
            let channels = stream.channels.get() as usize;
            let mut count = 0;
            let mut peak = 0f32;
            while let Ok(block) = stream.receiver.recv_timeout(Duration::from_secs(5)) {
                assert!(block.len() <= STREAM_FRAMES * channels);
                count += block.len();
                for sample in block {
                    peak = peak.max(sample.abs());
                }
            }
            assert!(
                count > rate * channels * 10,
                "track should contain over ten seconds"
            );
            assert!(count <= MAX_MUSIC_SECONDS * rate * channels);
            assert!(peak > 0.01);
            // Cancellation also works while a decoder is back-pressured by a
            // full queue. The sender owns no game/device resources.
            let cancelled = stream_music(&dir.join(file)).unwrap();
            drop(cancelled);
        }
    }
}
