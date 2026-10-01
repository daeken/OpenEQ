//! Bounded, offline MIDI synthesis using the installed macOS DLSSynth.
//!
//! Create, use and drop the synth inside the synthesis worker. This type is
//! deliberately neither Send nor Sync. It creates a music device only: no
//! output unit, audio graph, hardware device, sound-bank download or bank copy.
//! Event timing belongs to the scheduler; split renders at event boundaries.

use std::{fmt, marker::PhantomData, rc::Rc};

pub const CHANNELS: usize = 2;
pub const MAX_RENDER_FRAMES: usize = 4096;
/// Native Miles default output bound, including the F0/F7 framing bytes.
pub const MAX_SYSEX_BYTES: usize = 1536;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SynthError {
    UnsupportedPlatform,
    InvalidSampleRate(u32),
    InvalidMidiMessage,
    InvalidSysEx,
    InvalidRenderBuffer(usize),
    ComponentUnavailable,
    AudioUnit {
        operation: &'static str,
        status: i32,
    },
    UnexpectedOutputFormat,
    InvalidRenderedAudio,
    TimelineOverflow,
    Failed,
}

impl fmt::Display for SynthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => f.write_str("offline MIDI synthesis requires macOS"),
            Self::InvalidSampleRate(rate) => write!(f, "unsupported synthesis sample rate {rate}"),
            Self::InvalidMidiMessage => f.write_str("invalid MIDI channel message"),
            Self::InvalidSysEx => f.write_str("SysEx requires a complete bounded F0..F7 message"),
            Self::InvalidRenderBuffer(samples) => write!(
                f,
                "synthesis buffer has {samples} samples; expected stereo frames, at most {MAX_RENDER_FRAMES} frames"
            ),
            Self::ComponentUnavailable => f.write_str("installed macOS DLSSynth is unavailable"),
            Self::AudioUnit { operation, status } => {
                write!(f, "{operation} failed with AudioUnit status {status}")
            }
            Self::UnexpectedOutputFormat => {
                f.write_str("DLSSynth returned an unexpected PCM format")
            }
            Self::InvalidRenderedAudio => f.write_str("DLSSynth returned invalid PCM data"),
            Self::TimelineOverflow => {
                f.write_str("synthesis sample timeline exceeded exact precision")
            }
            Self::Failed => f.write_str("synth cannot be reused after a native rendering failure"),
        }
    }
}

impl std::error::Error for SynthError {}

pub struct MidiSynth {
    #[cfg(target_os = "macos")]
    inner: macos::Synth,
    _owning_thread: PhantomData<Rc<()>>,
}

impl MidiSynth {
    /// Instantiate on the synthesis worker, using the OS component's default bank.
    pub fn create(sample_rate: u32) -> Result<Self, SynthError> {
        if !(8_000..=192_000).contains(&sample_rate) {
            return Err(SynthError::InvalidSampleRate(sample_rate));
        }
        #[cfg(target_os = "macos")]
        {
            Ok(Self {
                inner: macos::Synth::create(sample_rate)?,
                _owning_thread: PhantomData,
            })
        }
        #[cfg(not(target_os = "macos"))]
        Err(SynthError::UnsupportedPlatform)
    }

    /// Apply a channel message at the next render boundary (sample offset zero).
    /// For program change/channel pressure, data2 must be zero. SysEx is rejected.
    pub fn midi_event(&mut self, status: u8, data1: u8, data2: u8) -> Result<(), SynthError> {
        validate_message(status, data1, data2)?;
        #[cfg(target_os = "macos")]
        return self.inner.midi_event(status, data1, data2);
        #[cfg(not(target_os = "macos"))]
        Err(SynthError::UnsupportedPlatform)
    }

    /// Forward a complete system-exclusive packet at the next render boundary.
    /// This preserves bytes; the installed synth determines instrument behavior.
    /// Split F7 continuation events and oversized packets are unsupported.
    pub fn sysex(&mut self, packet: &[u8]) -> Result<(), SynthError> {
        validate_sysex(packet)?;
        #[cfg(target_os = "macos")]
        return self.inner.sysex(packet);
        #[cfg(not(target_os = "macos"))]
        Err(SynthError::UnsupportedPlatform)
    }

    /// Fill a bounded caller-owned interleaved stereo f32 buffer.
    /// Empty renders are no-ops. Errors leave the caller's buffer silent.
    /// This operation allocates no Rust buffers and never opens audio output.
    pub fn render(&mut self, interleaved: &mut [f32]) -> Result<(), SynthError> {
        interleaved.fill(0.0);
        validate_buffer(interleaved.len())?;
        #[cfg(target_os = "macos")]
        return self.inner.render(interleaved);
        #[cfg(not(target_os = "macos"))]
        Err(SynthError::UnsupportedPlatform)
    }

    /// Dispose explicitly when shutdown errors should be reported. Drop also
    /// uninitializes/disposes, including every partially initialized error path.
    pub fn close(mut self) -> Result<(), SynthError> {
        #[cfg(target_os = "macos")]
        return self.inner.close();
        #[cfg(not(target_os = "macos"))]
        {
            let _ = &mut self;
            Err(SynthError::UnsupportedPlatform)
        }
    }
}

fn validate_message(status: u8, data1: u8, data2: u8) -> Result<(), SynthError> {
    if !(0x80..=0xef).contains(&status)
        || data1 >= 128
        || data2 >= 128
        || (matches!(status & 0xf0, 0xc0 | 0xd0) && data2 != 0)
    {
        return Err(SynthError::InvalidMidiMessage);
    }
    Ok(())
}

pub(crate) fn validate_sysex(packet: &[u8]) -> Result<(), SynthError> {
    let (&status, payload) = packet.split_first().ok_or(SynthError::InvalidSysEx)?;
    validate_sysex_payload(status, payload)
}

pub(crate) fn validate_sysex_payload(status: u8, payload: &[u8]) -> Result<(), SynthError> {
    if status != 0xf0
        || !(1..MAX_SYSEX_BYTES).contains(&payload.len())
        || payload.last() != Some(&0xf7)
        || payload[..payload.len() - 1].iter().any(|byte| *byte >= 128)
    {
        return Err(SynthError::InvalidSysEx);
    }
    Ok(())
}

fn validate_buffer(samples: usize) -> Result<(), SynthError> {
    if !samples.is_multiple_of(CHANNELS) || samples / CHANNELS > MAX_RENDER_FRAMES {
        return Err(SynthError::InvalidRenderBuffer(samples));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{CHANNELS, MAX_RENDER_FRAMES, SynthError};
    use std::{ffi::c_void, mem::size_of, ptr::NonNull, sync::Mutex};

    // DLSSynth instances share a native sound-bank cache whose final release
    // races with acquisition by another instance. Serialize creation/configuration
    // and teardown, while keeping rendering and MIDI on independent workers.
    // See docs/COREAUDIO_SYNTH_LIFECYCLE.md for the native crash evidence.
    static SYNTH_LIFECYCLE: Mutex<()> = Mutex::new(());

    // ABI verified against AudioComponent.h, AUComponent.h, MusicDevice.h,
    // AudioUnitProperties.h and CoreAudioBaseTypes.h in the local macOS SDK.
    // Every AudioUnit entry point below is synchronous; no callbacks retain
    // these Rust pointers. No output-component or audio-device API is declared.
    #[repr(C)]
    struct ComponentDescription {
        kind: u32,
        subtype: u32,
        manufacturer: u32,
        flags: u32,
        flags_mask: u32,
    }

    #[repr(C)]
    #[derive(Default, PartialEq)]
    struct StreamDescription {
        sample_rate: f64,
        format_id: u32,
        format_flags: u32,
        bytes_per_packet: u32,
        frames_per_packet: u32,
        bytes_per_frame: u32,
        channels_per_frame: u32,
        bits_per_channel: u32,
        reserved: u32,
    }

    #[repr(C)]
    struct AudioBuffer {
        channels: u32,
        bytes: u32,
        data: *mut c_void,
    }

    // AudioBufferList is a C variable-length array; two output planes require
    // storage for precisely two AudioBuffer records, including C alignment.
    #[repr(C)]
    struct StereoBufferList {
        count: u32,
        buffers: [AudioBuffer; CHANNELS],
    }

    #[repr(C)]
    #[derive(Default)]
    struct SmpteTime {
        subframes: i16,
        subframe_divisor: i16,
        counter: u32,
        kind: u32,
        flags: u32,
        hours: i16,
        minutes: i16,
        seconds: i16,
        frames: i16,
    }

    #[repr(C)]
    #[derive(Default)]
    struct TimeStamp {
        sample_time: f64,
        host_time: u64,
        rate_scalar: f64,
        word_clock_time: u64,
        smpte_time: SmpteTime,
        flags: u32,
        reserved: u32,
    }

    #[link(name = "AudioToolbox", kind = "framework")]
    unsafe extern "C" {
        fn AudioComponentFindNext(
            previous: *mut c_void,
            description: *const ComponentDescription,
        ) -> *mut c_void;
        fn AudioComponentInstanceNew(component: *mut c_void, instance: *mut *mut c_void) -> i32;
        fn AudioComponentInstanceDispose(unit: *mut c_void) -> i32;
        fn AudioUnitInitialize(unit: *mut c_void) -> i32;
        fn AudioUnitUninitialize(unit: *mut c_void) -> i32;
        fn AudioUnitSetProperty(
            unit: *mut c_void,
            property: u32,
            scope: u32,
            element: u32,
            data: *const c_void,
            size: u32,
        ) -> i32;
        fn AudioUnitGetProperty(
            unit: *mut c_void,
            property: u32,
            scope: u32,
            element: u32,
            data: *mut c_void,
            size: *mut u32,
        ) -> i32;
        fn MusicDeviceMIDIEvent(
            unit: *mut c_void,
            status: u32,
            data1: u32,
            data2: u32,
            sample_offset: u32,
        ) -> i32;
        fn MusicDeviceSysEx(unit: *mut c_void, data: *const u8, length: u32) -> i32;
        fn AudioUnitRender(
            unit: *mut c_void,
            flags: *mut u32,
            timestamp: *const TimeStamp,
            output_bus: u32,
            frames: u32,
            buffers: *mut StereoBufferList,
        ) -> i32;
    }

    const GLOBAL: u32 = 0;
    const OUTPUT: u32 = 2;
    const STREAM_FORMAT: u32 = 8;
    const MAX_FRAMES_PER_SLICE: u32 = 14;
    const OFFLINE_RENDER: u32 = 37;
    const FLOAT_PACKED_NONINTERLEAVED: u32 = 1 | 8 | 32;
    const SAMPLE_TIME_VALID: u32 = 1;

    #[repr(C, align(16))]
    struct AlignedSamples([[f32; MAX_RENDER_FRAMES]; CHANNELS]);

    pub(super) struct Synth {
        unit: Option<NonNull<c_void>>,
        initialized: bool,
        failed: bool,
        frame: u64,
        scratch: Box<AlignedSamples>,
    }

    fn check(operation: &'static str, status: i32) -> Result<(), SynthError> {
        if status == 0 {
            Ok(())
        } else {
            Err(SynthError::AudioUnit { operation, status })
        }
    }

    impl Synth {
        pub(super) fn create(sample_rate: u32) -> Result<Self, SynthError> {
            // Declare the owner before the guard: any early return or unwind
            // must release the lifecycle lock before Drop reacquires it.
            let mut synth = Self {
                unit: None,
                initialized: false,
                failed: false,
                frame: 0,
                scratch: Box::new(AlignedSamples([[0.0; MAX_RENDER_FRAMES]; CHANNELS])),
            };
            let _lifecycle = SYNTH_LIFECYCLE.lock().unwrap_or_else(|e| e.into_inner());
            let description = ComponentDescription {
                kind: u32::from_be_bytes(*b"aumu"),
                subtype: u32::from_be_bytes(*b"dls "),
                manufacturer: u32::from_be_bytes(*b"appl"),
                flags: 0,
                flags_mask: 0,
            };
            // SAFETY: The description is initialized and alive for this call.
            let component = unsafe { AudioComponentFindNext(std::ptr::null_mut(), &description) };
            if component.is_null() {
                return Err(SynthError::ComponentUnavailable);
            }
            let mut instance = std::ptr::null_mut();
            // SAFETY: Nonnull system component and writable instance pointer.
            check("AudioComponentInstanceNew", unsafe {
                AudioComponentInstanceNew(component, &mut instance)
            })?;
            let unit = NonNull::new(instance).ok_or(SynthError::ComponentUnavailable)?;
            synth.unit = Some(unit);
            let format = StreamDescription {
                sample_rate: sample_rate as f64,
                format_id: u32::from_be_bytes(*b"lpcm"),
                format_flags: FLOAT_PACKED_NONINTERLEAVED,
                bytes_per_packet: 4,
                frames_per_packet: 1,
                bytes_per_frame: 4,
                channels_per_frame: CHANNELS as u32,
                bits_per_channel: 32,
                reserved: 0,
            };
            synth.set_property(STREAM_FORMAT, OUTPUT, &format, "set synth PCM format")?;
            synth.set_property(
                MAX_FRAMES_PER_SLICE,
                GLOBAL,
                &(MAX_RENDER_FRAMES as u32),
                "set synth render limit",
            )?;
            synth.set_property(
                OFFLINE_RENDER,
                GLOBAL,
                &1_u32,
                "set synth offline rendering",
            )?;
            // SAFETY: All properties have the SDK-defined ABI and this live
            // instance is exclusively owned by the current thread.
            check("AudioUnitInitialize", unsafe {
                AudioUnitInitialize(unit.as_ptr())
            })?;
            synth.initialized = true;
            let mut actual = StreamDescription::default();
            let mut bytes = size_of::<StreamDescription>() as u32;
            // SAFETY: Writable format storage and exact byte size for property 8.
            check("get synth PCM format", unsafe {
                AudioUnitGetProperty(
                    unit.as_ptr(),
                    STREAM_FORMAT,
                    OUTPUT,
                    0,
                    (&mut actual as *mut StreamDescription).cast(),
                    &mut bytes,
                )
            })?;
            if bytes != size_of::<StreamDescription>() as u32 || actual != format {
                return Err(SynthError::UnexpectedOutputFormat);
            }
            Ok(synth)
        }

        // Private callers use only SDK-layout plain data for the named property.
        fn set_property<T>(
            &self,
            property: u32,
            scope: u32,
            value: &T,
            operation: &'static str,
        ) -> Result<(), SynthError> {
            // SAFETY: Live instance, correct property-specific storage/size;
            // AudioUnitSetProperty copies the data synchronously.
            check(operation, unsafe {
                AudioUnitSetProperty(
                    self.unit.expect("live synth").as_ptr(),
                    property,
                    scope,
                    0,
                    (value as *const T).cast(),
                    size_of::<T>() as u32,
                )
            })
        }

        pub(super) fn midi_event(
            &mut self,
            status: u8,
            data1: u8,
            data2: u8,
        ) -> Result<(), SynthError> {
            if self.failed {
                return Err(SynthError::Failed);
            }
            // SAFETY: Validated channel bytes, exclusively owned initialized
            // unit, and zero offset at this worker's next render boundary.
            let result = check("MusicDeviceMIDIEvent", unsafe {
                MusicDeviceMIDIEvent(
                    self.unit.expect("live synth").as_ptr(),
                    status as u32,
                    data1 as u32,
                    data2 as u32,
                    0,
                )
            });
            self.failed = result.is_err();
            result
        }

        pub(super) fn sysex(&mut self, packet: &[u8]) -> Result<(), SynthError> {
            if self.failed {
                return Err(SynthError::Failed);
            }
            // SAFETY: Validated complete framing and length <=1536; exclusively
            // owned initialized unit and SDK-verified ABI. The immutable slice
            // stays alive throughout the call; no callback is registered.
            let result = check("MusicDeviceSysEx", unsafe {
                MusicDeviceSysEx(
                    self.unit.expect("live synth").as_ptr(),
                    packet.as_ptr(),
                    packet.len() as u32,
                )
            });
            self.failed = result.is_err();
            result
        }

        pub(super) fn render(&mut self, interleaved: &mut [f32]) -> Result<(), SynthError> {
            if self.failed {
                return Err(SynthError::Failed);
            }
            let frames = interleaved.len() / CHANNELS;
            if frames == 0 {
                return Ok(());
            }
            let next = self
                .frame
                .checked_add(frames as u64)
                .filter(|&frame| frame <= 1_u64 << 53)
                .ok_or(SynthError::TimelineOverflow)?;
            let timestamp = TimeStamp {
                sample_time: self.frame as f64,
                flags: SAMPLE_TIME_VALID,
                ..TimeStamp::default()
            };
            let bytes = (frames * size_of::<f32>()) as u32;
            let mut buffers = StereoBufferList {
                count: CHANNELS as u32,
                buffers: std::array::from_fn(|channel| {
                    self.scratch.0[channel][..frames].fill(0.0);
                    AudioBuffer {
                        channels: 1,
                        bytes,
                        data: self.scratch.0[channel].as_mut_ptr().cast(),
                    }
                }),
            };
            let mut flags = 0;
            // SAFETY: Two 16-byte-aligned float planes match the verified ASBD;
            // both have space for frames <= MAX_RENDER_FRAMES. Pointers and
            // timestamp live throughout this synchronous call, with no aliasing.
            let result = check("AudioUnitRender", unsafe {
                AudioUnitRender(
                    self.unit.expect("live synth").as_ptr(),
                    &mut flags,
                    &timestamp,
                    0,
                    frames as u32,
                    &mut buffers,
                )
            });
            if let Err(error) = result {
                self.failed = true;
                return Err(error);
            }
            self.frame = next;
            if buffers.count != CHANNELS as u32
                || buffers.buffers.iter().enumerate().any(|(channel, buffer)| {
                    buffer.channels != 1
                        || buffer.bytes != bytes
                        || buffer.data != self.scratch.0[channel].as_mut_ptr().cast()
                        || self.scratch.0[channel][..frames]
                            .iter()
                            .any(|v| !v.is_finite())
                })
            {
                self.failed = true;
                return Err(SynthError::InvalidRenderedAudio);
            }
            for (frame, output) in interleaved.chunks_exact_mut(CHANNELS).enumerate() {
                output[0] = self.scratch.0[0][frame];
                output[1] = self.scratch.0[1][frame];
            }
            Ok(())
        }

        pub(super) fn close(&mut self) -> Result<(), SynthError> {
            let _lifecycle = SYNTH_LIFECYCLE.lock().unwrap_or_else(|e| e.into_inner());
            let Some(unit) = self.unit.take() else {
                return Ok(());
            };
            // SAFETY: Sole owning thread; no callbacks or other references exist.
            // Dispose is attempted even if uninitialization reports failure.
            let uninitialize = if self.initialized {
                self.initialized = false;
                check("AudioUnitUninitialize", unsafe {
                    AudioUnitUninitialize(unit.as_ptr())
                })
            } else {
                Ok(())
            };
            let dispose = check("AudioComponentInstanceDispose", unsafe {
                AudioComponentInstanceDispose(unit.as_ptr())
            });
            uninitialize.and(dispose)
        }
    }

    impl Drop for Synth {
        fn drop(&mut self) {
            let _ = self.close();
        }
    }

    #[test]
    fn sdk_abi_layouts_match() {
        use std::mem::{align_of, offset_of};
        assert_eq!(size_of::<ComponentDescription>(), 20);
        assert_eq!(size_of::<StreamDescription>(), 40);
        assert_eq!(size_of::<SmpteTime>(), 24);
        assert_eq!(size_of::<TimeStamp>(), 64);
        assert_eq!(offset_of!(TimeStamp, flags), 56);
        assert_eq!(size_of::<AudioBuffer>(), 16);
        assert_eq!(size_of::<StereoBufferList>(), 40);
        assert_eq!(offset_of!(StereoBufferList, buffers), 8);
        assert_eq!(align_of::<AlignedSamples>(), 16);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sysex_requires_bounded_complete_packets_without_channel_status_bytes() {
        assert!(validate_sysex(&[0xf0, 0xf7]).is_ok());
        assert!(validate_sysex(&[0xf0, 0x7e, 0x7f, 9, 1, 0xf7]).is_ok());
        for packet in [
            &[][..],
            &[0xf0],
            &[0xf7, 1, 0xf7],
            &[0xf0, 1, 0],
            &[0xf0, 0x80, 0xf7],
            &[0xf0, 0xf7, 0xf7],
            &[0x90, 60, 0],
        ] {
            assert_eq!(validate_sysex(packet), Err(SynthError::InvalidSysEx));
        }
        let mut packet = vec![0x7f; MAX_SYSEX_BYTES];
        packet[0] = 0xf0;
        packet[MAX_SYSEX_BYTES - 1] = 0xf7;
        assert!(validate_sysex(&packet).is_ok());
        packet.insert(1, 0);
        assert_eq!(validate_sysex(&packet), Err(SynthError::InvalidSysEx));
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires original The Deep XMI and offline OS DLSSynth; memory only, no output device"]
    fn offline_original_sysex_transport() {
        use openeq_assets::audio::xmi::{XmiEventKind, XmiFile};
        let base = std::path::PathBuf::from(std::env::var_os("EQ_DIR").expect("set EQ_DIR"));
        let file = XmiFile::parse(&std::fs::read(base.join("thedeep.xmi")).unwrap()).unwrap();
        let packets: Vec<_> = file.sequences[0]
            .events
            .iter()
            .filter_map(|event| {
                let XmiEventKind::SysEx { status, payload } = &event.kind else {
                    return None;
                };
                let packet: Vec<_> = std::iter::once(*status)
                    .chain(payload.iter().copied())
                    .collect();
                Some((event.tick, packet))
            })
            .collect();
        assert_eq!(packets.len(), 22);
        assert_eq!(packets.first().unwrap().0, 0);
        assert_eq!(packets.last().unwrap().0, 10);
        std::thread::spawn(move || {
            let mut synth = MidiSynth::create(48_000).expect("installed DLSSynth");
            let mut tick = 0;
            for (next_tick, packet) in packets {
                while tick < next_tick {
                    let mut samples = [0.; 400 * CHANNELS];
                    synth.render(&mut samples).unwrap();
                    assert!(samples.iter().all(|sample| sample.is_finite()));
                    tick += 1;
                }
                synth.sysex(&packet).unwrap();
            }
            // Caller validation must not poison an otherwise healthy synth.
            assert_eq!(synth.sysex(&[0xf0]), Err(SynthError::InvalidSysEx));
            synth.midi_event(0xc0, 0, 0).unwrap();
            synth.midi_event(0x90, 60, 100).unwrap();
            let mut samples = [0.; MAX_RENDER_FRAMES * CHANNELS];
            synth.render(&mut samples).unwrap();
            assert!(samples.iter().all(|sample| sample.is_finite()));
            assert!(samples.iter().any(|sample| sample.abs() > 0.00001));
            synth.midi_event(0x80, 60, 0).unwrap();
            synth.close().unwrap();
        })
        .join()
        .unwrap();
    }

    #[test]
    fn channel_message_and_render_bounds() {
        for status in 0x80..=0xef {
            assert!(validate_message(status, 127, 0).is_ok());
        }
        for message in [
            (0x7f, 0, 0),
            (0xf0, 0, 0),
            (0x90, 128, 0),
            (0x90, 60, 128),
            (0xc0, 3, 1),
        ] {
            assert_eq!(
                validate_message(message.0, message.1, message.2),
                Err(SynthError::InvalidMidiMessage)
            );
        }
        for samples in [0, 2, MAX_RENDER_FRAMES * CHANNELS] {
            assert!(validate_buffer(samples).is_ok());
        }
        for samples in [1, MAX_RENDER_FRAMES * CHANNELS + 2, usize::MAX] {
            assert_eq!(
                validate_buffer(samples),
                Err(SynthError::InvalidRenderBuffer(samples))
            );
        }
        for rate in [0, 7_999, 192_001, u32::MAX] {
            assert!(matches!(
                MidiSynth::create(rate),
                Err(SynthError::InvalidSampleRate(_))
            ));
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn other_platforms_report_unsupported() {
        assert!(matches!(
            MidiSynth::create(48_000),
            Err(SynthError::UnsupportedPlatform)
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "opt-in: concurrent OS synth lifecycle stress, renders only to memory"]
    fn concurrent_offline_synth_lifecycle() {
        use std::sync::{Arc, Barrier};
        let barrier = Arc::new(Barrier::new(4));
        let workers: Vec<_> = (0..4)
            .map(|worker| {
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    // Only synchronize startup; a failed worker must not strand
                    // its peers at a later barrier. Each worker owns its synth.
                    barrier.wait();
                    for iteration in 0..32 {
                        let rate = [44_100, 48_000][(worker + iteration) % 2];
                        let mut synth = MidiSynth::create(rate).expect("offline concurrent synth");
                        synth.midi_event(0x90, 60 + worker as u8, 100).unwrap();
                        let mut samples = vec![0.; 256 * CHANNELS];
                        synth.render(&mut samples).unwrap();
                        assert!(samples.iter().all(|sample| sample.is_finite()));
                        if iteration % 2 == 0 {
                            synth.close().unwrap();
                        } else {
                            drop(synth);
                        }
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "opt-in: instantiates OS DLSSynth and renders synthetic notes to memory; no device output"]
    fn offline_synthetic_note_and_cleanup() {
        std::thread::spawn(|| {
            for rate in [44_100, 48_000] {
                let mut synth = MidiSynth::create(rate).expect("installed DLSSynth");
                synth.midi_event(0xc0, 0, 0).unwrap();
                synth.midi_event(0xb0, 7, 100).unwrap();
                synth.midi_event(0xb0, 11, 127).unwrap();
                synth.midi_event(0x90, 60, 100).unwrap();
                let mut peak = 0.0_f32;
                for frames in [1, 7, 128, 511, MAX_RENDER_FRAMES, MAX_RENDER_FRAMES] {
                    let mut samples = vec![f32::NAN; frames * CHANNELS];
                    synth.render(&mut samples).unwrap();
                    assert!(samples.iter().all(|sample| sample.is_finite()));
                    peak = samples
                        .iter()
                        .map(|sample| sample.abs())
                        .fold(peak, f32::max);
                }
                assert!(peak > 0.00001, "synthetic note was silent at {rate} Hz");
                synth.midi_event(0x80, 60, 0).unwrap();
                synth.midi_event(0xb0, 64, 0).unwrap();
                let mut samples = vec![0.0; MAX_RENDER_FRAMES * CHANNELS];
                for _ in 0..12 {
                    synth.render(&mut samples).unwrap();
                    assert!(samples.iter().all(|sample| sample.is_finite()));
                }
                let mut too_large = vec![1.0; (MAX_RENDER_FRAMES + 1) * CHANNELS];
                assert!(matches!(
                    synth.render(&mut too_large),
                    Err(SynthError::InvalidRenderBuffer(_))
                ));
                assert!(too_large.iter().all(|&sample| sample == 0.0));
                synth.render(&mut []).unwrap();
                synth.close().expect("explicit cleanup");
            }
            // Exercise Drop cleanup independently of explicit close.
            drop(MidiSynth::create(48_000).unwrap());
        })
        .join()
        .unwrap();
    }
}
