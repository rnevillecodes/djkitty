//! Audio output device discovery.

use cpal::traits::{DeviceTrait, HostTrait};

/// One supported configuration range of an output device.
#[derive(Debug, Clone)]
pub struct ConfigRange {
    pub channels: u16,
    pub min_sample_rate: u32,
    pub max_sample_rate: u32,
    /// `None` when the host cannot report buffer sizes before opening a stream.
    pub buffer_frames: Option<(u32, u32)>,
    pub sample_format: String,
}

/// An output device and everything it reports supporting.
#[derive(Debug, Clone)]
pub struct OutputDeviceInfo {
    pub name: String,
    pub is_default: bool,
    pub default_config: Option<(u16, u32)>,
    pub configs: Vec<ConfigRange>,
}

impl OutputDeviceInfo {
    /// The largest channel count across all supported configurations.
    pub fn max_channels(&self) -> u16 {
        self.configs
            .iter()
            .map(|c| c.channels)
            .chain(self.default_config.map(|(c, _)| c))
            .max()
            .unwrap_or(0)
    }
}

/// Human-readable name of a cpal device.
pub fn device_name(device: &cpal::Device) -> String {
    device
        .description()
        .map(|d| d.name().to_owned())
        .unwrap_or_else(|_| "<unknown device>".to_owned())
}

/// List every output device on the default host.
pub fn list_output_devices() -> Result<Vec<OutputDeviceInfo>, cpal::Error> {
    let host = cpal::default_host();
    let default_name = host.default_output_device().map(|d| device_name(&d));
    let mut out = Vec::new();
    for device in host.output_devices()? {
        let name = device_name(&device);
        let default_config = device
            .default_output_config()
            .ok()
            .map(|c| (c.channels(), c.sample_rate()));
        let configs = device
            .supported_output_configs()
            .map(|it| {
                it.map(|c| ConfigRange {
                    channels: c.channels(),
                    min_sample_rate: c.min_sample_rate(),
                    max_sample_rate: c.max_sample_rate(),
                    buffer_frames: match c.buffer_size() {
                        cpal::SupportedBufferSize::Range { min, max } => Some((*min, *max)),
                        _ => None,
                    },
                    sample_format: format!("{:?}", c.sample_format()),
                })
                .collect()
            })
            .unwrap_or_default();
        out.push(OutputDeviceInfo {
            is_default: default_name.as_deref() == Some(name.as_str()),
            name,
            default_config,
            configs,
        });
    }
    Ok(out)
}
