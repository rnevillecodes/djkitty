//! The app's view of the audio engine: owns the engine, its handles and the output stream.

use std::sync::{Arc, Mutex};

use djk_audio::output::{self, OutputError};
use djk_audio::{AudioOutput, Engine, EngineHandles, SharedEngine};
use djk_core::{Command, EngineState, LoadRequest};

pub struct AudioService {
    engine: SharedEngine,
    pub handles: EngineHandles,
    pub output: Option<AudioOutput>,
    pub error: Option<String>,
    /// Latest snapshot, refreshed by [`AudioService::poll`].
    pub state: EngineState,
}

impl AudioService {
    /// Create the engine and open the preferred device (DDJ-FLX2, saved choice, default).
    pub fn start(saved_device: Option<&str>) -> Self {
        let (engine, handles) = Engine::new(48_000, 2);
        let mut svc = Self {
            engine: Arc::new(Mutex::new(engine)),
            handles,
            output: None,
            error: None,
            state: EngineState::default(),
        };
        svc.open(saved_device.and_then(output::device_by_name), saved_device);
        svc
    }

    fn open(&mut self, device: Option<cpal::Device>, saved: Option<&str>) {
        // Stop any running stream before touching the engine.
        self.output = None;
        let result = match device {
            Some(d) => output::open_device(&d, self.engine.clone()),
            None => output::open(self.engine.clone(), saved),
        };
        match result {
            Ok(out) => {
                tracing::info!(
                    "audio: {} ({} ch @ {} Hz, buffer {:?})",
                    out.device_name,
                    out.channels,
                    out.sample_rate,
                    out.buffer_frames
                );
                self.output = Some(out);
                self.error = None;
            }
            Err(e) => {
                tracing::error!("audio: {e}");
                self.error = Some(e.to_string());
            }
        }
    }

    /// Switch to a device by name (or the automatic choice when `None`).
    pub fn switch_device(&mut self, name: Option<&str>) {
        let device = name.and_then(output::device_by_name);
        if name.is_some() && device.is_none() {
            self.error = Some(OutputError::NoDevice.to_string());
            return;
        }
        self.open(device, None);
    }

    pub fn is_disconnected(&self) -> bool {
        self.output.as_ref().is_none_or(|o| o.is_disconnected())
    }

    /// Refresh the snapshot and free retired buffers. Call once per UI frame.
    pub fn poll(&mut self) {
        self.state = *self.handles.state.read();
        self.handles.drop_retired();
    }

    pub fn send(&mut self, cmd: Command) {
        if !self.handles.send(cmd) {
            tracing::warn!("engine command queue full, dropped {cmd:?}");
        }
    }

    pub fn load(&mut self, req: LoadRequest) -> bool {
        self.handles.load(req).is_ok()
    }
}
