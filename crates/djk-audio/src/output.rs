//! cpal output stream: device selection, format choice and the audio callback.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::denormal::NoDenormals;
use crate::device::device_name;
use crate::engine::Engine;

/// The engine shared between the audio callback and the app. The callback only ever uses
/// `try_lock` (never blocks); the app locks it only while no stream is running it, e.g.
/// when switching devices.
pub type SharedEngine = Arc<Mutex<Engine>>;

/// Substring identifying the DDJ-FLX2's audio device.
pub const PREFERRED_DEVICE: &str = "DDJ-FLX2";

#[derive(Debug, thiserror::Error)]
pub enum OutputError {
    #[error("no audio output device available")]
    NoDevice,
    #[error("device does not support 32-bit float output")]
    NoFloatConfig,
    #[error("audio device error: {0}")]
    Device(String),
}

/// A running output stream.
pub struct AudioOutput {
    _stream: cpal::Stream,
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
    /// Requested buffer size in frames, when the device reports one.
    pub buffer_frames: Option<u32>,
    disconnected: Arc<AtomicBool>,
}

impl AudioOutput {
    /// True once the stream reported an error (typically the device was unplugged).
    pub fn is_disconnected(&self) -> bool {
        self.disconnected.load(Ordering::Relaxed)
    }
}

/// Choose an output device: one whose name contains [`PREFERRED_DEVICE`], else the saved
/// choice, else the system default.
pub fn select_device(saved: Option<&str>) -> Option<cpal::Device> {
    let host = cpal::default_host();
    let devices: Vec<cpal::Device> = host.output_devices().ok()?.collect();
    let find = |needle: &str| {
        devices
            .iter()
            .find(|d| device_name(d).contains(needle))
            .cloned()
    };
    find(PREFERRED_DEVICE)
        .or_else(|| saved.and_then(|s| devices.iter().find(|d| device_name(d) == s).cloned()))
        .or_else(|| host.default_output_device())
}

/// Find an output device by exact name.
pub fn device_by_name(name: &str) -> Option<cpal::Device> {
    cpal::default_host()
        .output_devices()
        .ok()?
        .find(|d| device_name(d) == name)
}

/// Pick the stream config: f32, 4 channels if available (master + headphones), else 2
/// (else whatever the device has), at the device's default rate, smallest buffer.
fn choose_config(device: &cpal::Device) -> Result<(cpal::StreamConfig, Option<u32>), OutputError> {
    let default_rate = device
        .default_output_config()
        .map(|c| c.sample_rate())
        .unwrap_or(48_000);
    let configs: Vec<_> = device
        .supported_output_configs()
        .map_err(|e| OutputError::Device(e.to_string()))?
        .filter(|c| c.sample_format() == cpal::SampleFormat::F32)
        .collect();
    let covers = |c: &cpal::SupportedStreamConfigRange, rate: u32| {
        c.min_sample_rate() <= rate && rate <= c.max_sample_rate()
    };
    let rank = |ch: u16| match ch {
        4 => 0,
        2 => 1,
        c if c > 4 => 2,
        _ => 3,
    };
    let best = configs
        .iter()
        .filter(|c| covers(c, default_rate))
        .min_by_key(|c| rank(c.channels()))
        .or_else(|| configs.iter().min_by_key(|c| rank(c.channels())))
        .ok_or(OutputError::NoFloatConfig)?;
    let rate = if covers(best, default_rate) {
        default_rate
    } else {
        best.max_sample_rate()
    };
    let buffer_frames = match best.buffer_size() {
        cpal::SupportedBufferSize::Range { min, max } => Some((*min).clamp(64, *max.max(&64))),
        _ => None,
    };
    let config = cpal::StreamConfig {
        channels: best.channels(),
        sample_rate: rate,
        buffer_size: buffer_frames
            .map(cpal::BufferSize::Fixed)
            .unwrap_or(cpal::BufferSize::Default),
    };
    Ok((config, buffer_frames))
}

/// Open `device` and start running `engine` on it.
pub fn open_device(
    device: &cpal::Device,
    engine: SharedEngine,
) -> Result<AudioOutput, OutputError> {
    let (mut config, buffer_frames) = choose_config(device)?;
    {
        let mut e = engine.lock().unwrap_or_else(|p| p.into_inner());
        e.set_output(config.sample_rate, config.channels);
    }
    let disconnected = Arc::new(AtomicBool::new(false));
    let stream = match build(device, &config, engine.clone(), disconnected.clone()) {
        Ok(s) => s,
        Err(_) if config.buffer_size != cpal::BufferSize::Default => {
            // Some hosts reject an explicit size; fall back to the device default.
            config.buffer_size = cpal::BufferSize::Default;
            build(device, &config, engine, disconnected.clone())?
        }
        Err(e) => return Err(e),
    };
    stream
        .play()
        .map_err(|e| OutputError::Device(e.to_string()))?;
    Ok(AudioOutput {
        _stream: stream,
        device_name: device_name(device),
        sample_rate: config.sample_rate,
        channels: config.channels,
        buffer_frames: match config.buffer_size {
            cpal::BufferSize::Fixed(n) => Some(n),
            cpal::BufferSize::Default => buffer_frames,
        },
        disconnected,
    })
}

/// Select a device (see [`select_device`]) and open it.
pub fn open(engine: SharedEngine, saved: Option<&str>) -> Result<AudioOutput, OutputError> {
    let device = select_device(saved).ok_or(OutputError::NoDevice)?;
    open_device(&device, engine)
}

fn build(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    engine: SharedEngine,
    disconnected: Arc<AtomicBool>,
) -> Result<cpal::Stream, OutputError> {
    let channels = usize::from(config.channels);
    let rate = f64::from(config.sample_rate);
    let mut last_callback: Option<Instant> = None;
    let on_error = move |err: cpal::Error| {
        // Runs on a cpal thread, not the real-time callback.
        tracing::warn!("audio stream error: {err}");
        disconnected.store(true, Ordering::Relaxed);
    };
    device
        .build_output_stream::<f32, _, _>(
            *config,
            move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                let now = Instant::now();
                let frames = data.len() / channels.max(1);
                let block = Duration::from_secs_f64(frames as f64 / rate);
                let ts = info.timestamp();
                let device_latency = ts.playback.duration_since(ts.callback);
                let overrun = last_callback.is_some_and(|prev| {
                    now.duration_since(prev) > block * 2 + Duration::from_millis(2)
                });
                last_callback = Some(now);
                match engine.try_lock() {
                    Ok(mut e) => {
                        // Filters decaying into silence must not hit the slow denormal path.
                        let _no_denormals = NoDenormals::enter();
                        if overrun {
                            e.note_overrun();
                        }
                        // The published position is the end of this block, heard after the
                        // device latency plus the block itself.
                        let latency = (device_latency + block).as_secs_f64();
                        let rendered_at = e.output_timing(now, latency, frames);
                        e.process(data, channels, Some(rendered_at));
                    }
                    Err(_) => data.fill(0.0),
                }
            },
            on_error,
            None,
        )
        .map_err(|e| OutputError::Device(e.to_string()))
}
