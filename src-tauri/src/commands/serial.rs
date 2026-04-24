//! Serial connection commands. Covers the whole session lifecycle
//! (connect / disconnect / auto-reconnect), the in-session controls
//! (send, DTR / RTS / break), port enumeration, and missing-driver
//! detection. Events are emitted for data, disconnects, reconnect
//! state changes, and transfer progress.

use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;
use chrono::Local;
use tauri::{AppHandle, Emitter, State};

use crate::appdata;
use crate::events;
use crate::profiles::Profile;
use crate::sanitize::SanitizingLogWriter;
use crate::serial::{self, Config, ControlLines, OnExit, OnRead, Session};
use crate::state::AppState;

const RECONNECT_INTERVAL: Duration = Duration::from_secs(1);
const RECONNECT_TIMEOUT: Duration = Duration::from_secs(30);

#[tauri::command]
pub fn list_ports() -> Result<Vec<serial::PortInfo>, String> {
    serial::list_ports().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_missing_drivers() -> Result<Vec<serial::USBSerialCandidate>, String> {
    serial::detect_missing_drivers()
}

#[tauri::command]
pub fn connect(
    profile_id: String,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let profile = state
        .profiles
        .get(&profile_id)
        .ok_or_else(|| format!("profile {} not found", profile_id))?;
    open_session(&app, state.inner(), profile).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn disconnect(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let (sess, cancel) = {
        let mut guard = state.session.lock().unwrap();
        let sess = guard.session.take();
        let cancel = guard.reconnect_cancel.take();
        guard.profile_id.clear();
        guard.profile_snapshot = None;
        (sess, cancel)
    };
    // Cancel any in-flight reconnect loop first — otherwise it might
    // briefly re-open the port between our Close and the user's
    // expectation that the port is free.
    if let Some(flag) = cancel {
        flag.store(true, Ordering::Release);
    }
    if let Some(sess) = sess {
        sess.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn send(data: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|e| format!("decode send payload: {}", e))?;
    let sess = active_session(state.inner()).ok_or("not connected")?;
    sess.send(&bytes).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_rts(v: bool, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let sess = active_session(state.inner()).ok_or("not connected")?;
    sess.set_rts(v).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_dtr(v: bool, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let sess = active_session(state.inner()).ok_or("not connected")?;
    sess.set_dtr(v).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn send_break(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let sess = active_session(state.inner()).ok_or("not connected")?;
    sess.send_break(serial::session::BREAK_DURATION)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn active_profile_id(state: State<'_, Arc<AppState>>) -> String {
    state.session.lock().unwrap().profile_id.clone()
}

#[tauri::command]
pub fn get_control_lines(state: State<'_, Arc<AppState>>) -> Result<ControlLines, String> {
    let sess = active_session(state.inner()).ok_or("not connected")?;
    Ok(sess.control_lines())
}

// --- Internals ---------------------------------------------------------

fn active_session(state: &Arc<AppState>) -> Option<Arc<Session>> {
    state.session.lock().unwrap().session.clone()
}

/// Open a session for `profile`, wire data / exit callbacks, and
/// install the Session in shared state. Shared between the initial
/// Connect command and the auto-reconnect loop so both paths produce
/// identical sessions.
fn open_session(
    app: &AppHandle,
    state: &Arc<AppState>,
    profile: Profile,
) -> Result<(), serial::SessionError> {
    {
        let guard = state.session.lock().unwrap();
        if guard.session.is_some() {
            return Err(serial::SessionError::Io(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "already connected — disconnect first",
            )));
        }
    }

    let cfg = Config {
        port_name: profile.port_name.clone(),
        baud_rate: profile.baud_rate as u32,
        data_bits: profile.data_bits as u32,
        parity: profile.parity.clone(),
        stop_bits: profile.stop_bits.clone(),
        flow_control: profile.flow_control.clone(),
        dtr_on_connect: profile.dtr_on_connect.clone(),
        rts_on_connect: profile.rts_on_connect.clone(),
        dtr_on_disconnect: profile.dtr_on_disconnect.clone(),
        rts_on_disconnect: profile.rts_on_disconnect.clone(),
    };

    let on_read = build_on_read(app.clone());
    let on_exit = build_on_exit(app.clone(), Arc::clone(state));

    let sess = Arc::new(Session::open(cfg, on_read, on_exit)?);

    if profile.log_enabled {
        match open_session_log(state, &profile) {
            Ok(writer) => sess.set_log_writer(Some(writer)),
            Err(err) => log::error!("open session log: {}", err),
        }
    }

    {
        let mut guard = state.session.lock().unwrap();
        guard.session = Some(Arc::clone(&sess));
        guard.profile_id = profile.id.clone();
        guard.profile_snapshot = Some(profile);
    }
    Ok(())
}

fn build_on_read(app: AppHandle) -> OnRead {
    Arc::new(move |bytes: &[u8]| {
        let payload = base64::engine::general_purpose::STANDARD.encode(bytes);
        let _ = app.emit(events::SERIAL_DATA, payload);
    })
}

fn build_on_exit(app: AppHandle, state: Arc<AppState>) -> OnExit {
    Arc::new(move |err: io::Error| {
        handle_session_exit(&app, &state, err);
    })
}

/// Runs on the read-pump thread when the port returns an error. On
/// profiles that opt into auto-reconnect it kicks off a retry loop;
/// otherwise it clears session state and emits `serial:disconnect`.
fn handle_session_exit(app: &AppHandle, state: &Arc<AppState>, err: io::Error) {
    let (old_session, profile) = {
        let mut guard = state.session.lock().unwrap();
        let sess = guard.session.take();
        let profile = guard.profile_snapshot.clone();
        (sess, profile)
    };

    // Close the orphaned session on a fresh thread — Session::close
    // joins the read-pump thread, and we're ON that thread right now.
    if let Some(sess) = old_session {
        thread::spawn(move || {
            let _ = sess.close();
        });
    }

    if let Some(profile) = profile {
        if profile.auto_reconnect {
            start_reconnect(app, state, profile);
            return;
        }
    }

    {
        let mut guard = state.session.lock().unwrap();
        guard.profile_id.clear();
        guard.profile_snapshot = None;
    }
    let _ = app.emit(events::SERIAL_DISCONNECT, err.to_string());
}

fn start_reconnect(app: &AppHandle, state: &Arc<AppState>, profile: Profile) {
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut guard = state.session.lock().unwrap();
        if let Some(prev) = guard.reconnect_cancel.take() {
            prev.store(true, Ordering::Release);
        }
        guard.reconnect_cancel = Some(Arc::clone(&cancel));
    }

    let _ = app.emit(events::SERIAL_RECONNECTING, profile.port_name.clone());

    let app = app.clone();
    let state = Arc::clone(state);
    thread::Builder::new()
        .name("baudrun-reconnect".into())
        .spawn(move || {
            let deadline = Instant::now() + RECONNECT_TIMEOUT;
            loop {
                thread::sleep(RECONNECT_INTERVAL);
                if cancel.load(Ordering::Acquire) {
                    finish_failed_reconnect(&app, &state, "reconnect cancelled");
                    return;
                }
                if Instant::now() >= deadline {
                    finish_failed_reconnect(&app, &state, "reconnect timeout");
                    return;
                }
                if open_session(&app, &state, profile.clone()).is_ok() {
                    let _ = app.emit(events::SERIAL_RECONNECTED, profile.id.clone());
                    let mut guard = state.session.lock().unwrap();
                    guard.reconnect_cancel = None;
                    return;
                }
            }
        })
        .expect("spawn baudrun-reconnect thread");
}

fn finish_failed_reconnect(app: &AppHandle, state: &Arc<AppState>, reason: &str) {
    {
        let mut guard = state.session.lock().unwrap();
        guard.profile_id.clear();
        guard.profile_snapshot = None;
        guard.reconnect_cancel = None;
    }
    let _ = app.emit(events::SERIAL_DISCONNECT, reason);
}

fn open_session_log(
    state: &Arc<AppState>,
    profile: &Profile,
) -> Result<Box<dyn Write + Send>, io::Error> {
    let mut dir = state.settings.get().log_dir;
    if dir.is_empty() {
        let support = appdata::support_dir().map_err(io::Error::other)?;
        dir = support.join("logs").to_string_lossy().into_owned();
    }
    let dir_path = PathBuf::from(&dir);
    std::fs::create_dir_all(&dir_path)?;
    let stamp = Local::now().format("%Y-%m-%d_%H%M%S");
    let filename = format!("{}_{}.log", slugify_session_name(&profile.name), stamp);
    let file = File::create(dir_path.join(filename))?;
    // Wrap so raw ANSI escapes + Cisco-style \r\r\n don't pollute
    // the plain-text log — we want it to read like the xterm view.
    Ok(Box::new(SanitizingLogWriter::new(file)))
}

/// Minimal slugify for log filenames — matches the Go version's
/// behaviour (lowercase alnum + `[ \-_.]` → dash, trim trailing
/// dashes, fall back to "session").
fn slugify_session_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars().flat_map(|c| c.to_lowercase()) {
        match ch {
            'a'..='z' | '0'..='9' => out.push(ch),
            ' ' | '-' | '_' | '.' => out.push('-'),
            _ => {}
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "session".into()
    } else {
        trimmed
    }
}
