//! Controller discovery, hot-plug and the MIDI input callback.
//!
//! A background thread polls the MIDI port list every second. When a port matching the
//! mapping appears it connects: the input callback translates messages and pushes engine
//! commands straight onto the engine's controller queue (never via the UI thread), and
//! sends library actions (browse/load) to the UI. When the port disappears it disconnects
//! and keeps the engine queue for the next connection.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use djk_core::{Command, EngineState};
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

use crate::led::{self, LedState};
use crate::mapping::{Action, Mapping, Translator};
use crate::takeover::{AppValues, Takeover};

/// How often the port list is polled for hot-plug.
pub const POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControllerEvent {
    Connected(String),
    Disconnected,
}

type Notify = Arc<dyn Fn() + Send + Sync>;

type EngineQueue = Arc<Mutex<rtrb::Producer<Command>>>;

struct CallbackData {
    translator: Translator,
    /// Shared so it survives failed connects (midir drops callback data on error).
    engine: EngineQueue,
    ui: Sender<Action>,
    notify: Notify,
}

fn on_message(_stamp: u64, msg: &[u8], data: &mut CallbackData) {
    match data.translator.translate(msg) {
        Some(Action::Engine(cmd)) => {
            if let Ok(mut q) = data.engine.lock() {
                let _ = q.push(cmd);
            }
            (data.notify)();
        }
        Some(action) => {
            let _ = data.ui.send(action);
            (data.notify)();
        }
        None => {}
    }
}

pub struct ControllerManager {
    mapping: &'static Mapping,
    status: Arc<Mutex<Option<String>>>,
    output: Arc<Mutex<Option<MidiOutputConnection>>>,
    actions: Receiver<Action>,
    events: Receiver<ControllerEvent>,
    /// App-side fader values for soft takeover; update with `set_from_ui` on screen changes.
    pub values: Arc<AppValues>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    last_leds: Option<LedState>,
}

impl ControllerManager {
    /// Start watching for the controller described by `mapping`.
    pub fn start(
        mapping: &'static Mapping,
        engine: rtrb::Producer<Command>,
        notify: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let status = Arc::new(Mutex::new(None));
        let output = Arc::new(Mutex::new(None));
        let values = Arc::new(AppValues::default());
        let stop = Arc::new(AtomicBool::new(false));
        let (action_tx, action_rx) = channel();
        let (event_tx, event_rx) = channel();
        let notify: Notify = Arc::new(notify);

        let thread = {
            let status = status.clone();
            let output = output.clone();
            let values = values.clone();
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("djk-controller".into())
                .spawn(move || {
                    watch(
                        mapping, engine, values, status, output, action_tx, event_tx, notify, stop,
                    )
                })
                .expect("spawn controller thread")
        };
        Self {
            mapping,
            status,
            output,
            actions: action_rx,
            events: event_rx,
            values,
            stop,
            thread: Some(thread),
            last_leds: None,
        }
    }

    /// Name of the connected port, if any.
    pub fn connected_port(&self) -> Option<String> {
        self.status.lock().unwrap().clone()
    }

    pub fn mapping(&self) -> &'static Mapping {
        self.mapping
    }

    /// Connection events since the last call. A `Connected` event schedules a full LED
    /// resync on the next [`ControllerManager::update_leds`].
    pub fn poll_events(&mut self) -> Vec<ControllerEvent> {
        let events: Vec<_> = self.events.try_iter().collect();
        if events
            .iter()
            .any(|e| matches!(e, ControllerEvent::Connected(_)))
        {
            self.last_leds = None;
        }
        events
    }

    /// Library actions (browse/load) from the controller.
    pub fn try_action(&self) -> Option<Action> {
        self.actions.try_recv().ok()
    }

    /// Send LED changes for the current engine state (full resync after connecting).
    pub fn update_leds(&mut self, state: &EngineState, time_secs: f64) {
        let mut guard = self.output.lock().unwrap();
        let Some(out) = guard.as_mut() else {
            self.last_leds = None;
            return;
        };
        let want = led::desired(state, &self.values, time_secs);
        if self.last_leds == Some(want) {
            return;
        }
        for msg in led::messages(self.mapping, self.last_leds.as_ref(), &want) {
            if out.send(&msg).is_err() {
                // Connection is going away; the watcher will notice.
                self.last_leds = None;
                return;
            }
        }
        self.last_leds = Some(want);
    }
}

impl Drop for ControllerManager {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn watch(
    mapping: &'static Mapping,
    engine: rtrb::Producer<Command>,
    values: Arc<AppValues>,
    status: Arc<Mutex<Option<String>>>,
    output: Arc<Mutex<Option<MidiOutputConnection>>>,
    actions: Sender<Action>,
    events: Sender<ControllerEvent>,
    notify: Notify,
    stop: Arc<AtomicBool>,
) {
    let engine: EngineQueue = Arc::new(Mutex::new(engine));
    let mut conn: Option<(MidiInputConnection<CallbackData>, String)> = None;
    while !stop.load(Ordering::Relaxed) {
        let ports = MidiInput::new("djkitty-scan").ok().map(|scan| {
            scan.ports()
                .iter()
                .filter_map(|p| scan.port_name(p).ok())
                .collect::<Vec<_>>()
        });

        if let Some((_, name)) = &conn {
            let present = ports.as_ref().is_some_and(|p| p.contains(name));
            if !present {
                let (c, name) = conn.take().unwrap();
                c.close();
                *output.lock().unwrap() = None;
                *status.lock().unwrap() = None;
                tracing::info!("controller disconnected: {name}");
                let _ = events.send(ControllerEvent::Disconnected);
                notify();
            }
        }

        if conn.is_none()
            && let Some(name) = ports
                .unwrap_or_default()
                .into_iter()
                .find(|n| n.contains(mapping.port_match))
        {
            match connect(mapping, &name, engine.clone(), &values, &actions, &notify) {
                Ok((input, out)) => {
                    *output.lock().unwrap() = out;
                    *status.lock().unwrap() = Some(name.clone());
                    tracing::info!("controller connected: {name}");
                    let _ = events.send(ControllerEvent::Connected(name.clone()));
                    notify();
                    conn = Some((input, name));
                }
                Err(e) => tracing::warn!("cannot open controller {name}: {e}"),
            }
        }

        // Sleep in small steps so shutdown is prompt.
        for _ in 0..10 {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(POLL_INTERVAL / 10);
        }
    }
    if let Some((c, _)) = conn {
        c.close();
    }
}

type Connected = (
    MidiInputConnection<CallbackData>,
    Option<MidiOutputConnection>,
);

fn connect(
    mapping: &'static Mapping,
    name: &str,
    engine: EngineQueue,
    values: &Arc<AppValues>,
    actions: &Sender<Action>,
    notify: &Notify,
) -> Result<Connected, String> {
    let input = match MidiInput::new("djkitty") {
        Ok(i) => i,
        Err(e) => return Err(e.to_string()),
    };
    let Some(port) = input
        .ports()
        .into_iter()
        .find(|p| input.port_name(p).ok().as_deref() == Some(name))
    else {
        return Err("port vanished".into());
    };
    let data = CallbackData {
        translator: Translator::new(mapping, Takeover::new(values.clone())),
        engine,
        ui: actions.clone(),
        notify: notify.clone(),
    };
    let input = input
        .connect(&port, "djkitty-in", on_message, data)
        .map_err(|e| e.to_string())?;
    let mut out = MidiOutput::new("djkitty-out").ok().and_then(|o| {
        let port = o
            .ports()
            .into_iter()
            .find(|p| o.port_name(p).is_ok_and(|n| n.contains(mapping.port_match)))?;
        o.connect(&port, "djkitty-out").ok()
    });
    match (&mut out, mapping.request_state) {
        (None, _) => tracing::warn!("controller output port not available; LEDs disabled"),
        // The input is already listening, so the positions it reports are applied.
        (Some(o), Some(msg)) => {
            if let Err(e) = o.send(msg) {
                tracing::warn!("cannot request controller state: {e}");
            }
        }
        (Some(_), None) => {}
    }
    Ok((input, out))
}
