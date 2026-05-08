//! Serial connection commands. Covers the whole session lifecycle
//! (connect / disconnect / auto-reconnect), the in-session controls
//! (send, DTR / RTS / break), port enumeration, and missing-driver
//! detection. Events are emitted for data, disconnects, reconnect
//! state changes, and transfer progress.
//!
//! Every command takes a [`WebviewWindow`] so its session is scoped
//! to the calling window — clicking Connect in window A connects A's
//! session, not main's. Background threads (read pump, auto-reconnect
//! loop) carry the window label and emit via [`AppHandle::emit_to`]
//! so events land in the right webview.

use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;
use chrono::Local;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

use crate::appdata;
use crate::events;
use crate::profiles::Profile;
use crate::sanitize::SanitizingLogWriter;
use crate::serial::{self, Config, ControlLines, OnExit, OnRead, Session};
use crate::state::AppState;

const RECONNECT_INTERVAL: Duration = Duration::from_secs(1);
const RECONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Enumerate available serial ports. Marked `async` (with the body
/// pushed to `spawn_blocking`) because on Windows the underlying
/// `serialport::available_ports` call goes through SetupAPI, which
/// regularly takes hundreds of ms — sometimes >1s — to walk the
/// device tree. Tauri 2's IPC dispatcher can run sync commands on
/// its blocking pool, but the *response delivery* path back to the
/// renderer appears to share a thread with other in-flight responses,
/// so a slow `list_ports` would tail-block unrelated commands like
/// `take_pending_profile_id` (a HashMap lookup) sitting behind it.
/// Tracing of v0.9.5-alpha.x on Windows showed this exact pattern:
/// a profile-window open stalled ~500ms purely because `list_ports`
/// from the previous render was still in flight.
///
/// `spawn_blocking` runs the SetupAPI work on a Tokio blocking thread
/// while the IPC dispatcher (and the renderer-bound response thread)
/// stay free. The await here is on the JoinHandle, not on SetupAPI.
#[tauri::command]
pub async fn list_ports() -> Result<Vec<serial::PortInfo>, String> {
    tauri::async_runtime::spawn_blocking(|| serial::list_ports().map_err(|e| e.to_string()))
        .await
        .map_err(|e| format!("list_ports join: {}", e))?
}

/// Detect attached USB-serial devices whose vendor driver isn't
/// installed (CH340, CP210x, FTDI, …). Same Windows blocking-call
/// concern as `list_ports` — `detect_missing_drivers` uses the same
/// SetupAPI / udev / IOKit enumeration paths and was contributing to
/// the same dispatcher tail-block — so it gets the same treatment.
#[tauri::command]
pub async fn list_missing_drivers() -> Result<Vec<serial::USBSerialCandidate>, String> {
    tauri::async_runtime::spawn_blocking(serial::detect_missing_drivers)
        .await
        .map_err(|e| format!("list_missing_drivers join: {}", e))?
}

/// `async` for the same reason as `send` — connect opens the OS
/// serial port (potentially slow, especially first-open after
/// driver enumeration), and we don't want it parked on the WebView
/// main thread while it negotiates baud / parity / flow-control.
#[tauri::command]
pub async fn connect(
    profile_id: String,
    window: WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let profile = state
        .profiles
        .get(&profile_id)
        .ok_or_else(|| format!("profile {} not found", profile_id))?;
    open_session(&window, state.inner(), profile).map_err(|e| e.to_string())
}

/// `async` for symmetry with `connect`. Disconnect close-port can
/// also block briefly on driver flush.
#[tauri::command]
pub async fn disconnect(
    window: WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let (sess, cancel) = state.with_session(window.label(), |handle| {
        let sess = handle.session.take();
        let cancel = handle.reconnect_cancel.take();
        handle.profile_id.clear();
        handle.profile_snapshot = None;
        (sess, cancel)
    });
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

/// `async` is load-bearing: this command fires on EVERY keystroke
/// the user types. Tauri 2 dispatches sync commands on the WebView
/// main thread, so a sync `send` would serialize every keystroke
/// through the same thread that's also responsible for receiving
/// IPC events from Rust (incoming serial data, etc.) and for
/// rendering the UI. On macOS WKWebView this overhead is small
/// enough to be invisible, but on Windows-on-ARM (WebView2 under
/// emulation, plus UTM-style USB pass-through) the per-call IPC
/// latency stacks and produces multi-second typing lag. Making
/// the command async drops it onto the Tokio runtime so the main
/// thread stays free; the underlying `Session::send` is still a
/// blocking serialport write under a mutex, but that blocks an
/// async worker rather than the main thread, so other commands
/// and inbound events keep flowing.
#[tauri::command]
pub async fn send(
    data: String,
    window: WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|e| format!("decode send payload: {}", e))?;
    let sess = active_session(state.inner(), window.label()).ok_or("not connected")?;
    sess.send(&bytes).map_err(|e| e.to_string())
}

/// `async` for the same reason as `send` — see that command's doc
/// comment. Less critical for control-line toggles than for
/// keystrokes (DTR/RTS pulses are infrequent), but consistent
/// dispatching keeps a stuck `send` from queuing behind an
/// occasional `set_rts` or vice versa.
#[tauri::command]
pub async fn set_rts(
    v: bool,
    window: WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let sess = active_session(state.inner(), window.label()).ok_or("not connected")?;
    sess.set_rts(v).map_err(|e| e.to_string())
}

/// `async` for the same reason as `send` and `set_rts`.
#[tauri::command]
pub async fn set_dtr(
    v: bool,
    window: WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let sess = active_session(state.inner(), window.label()).ok_or("not connected")?;
    sess.set_dtr(v).map_err(|e| e.to_string())
}

/// `async` for the same reason as `send` — `Session::send_break`
/// holds the line low for a fixed duration (BREAK_DURATION), which
/// is ~250ms of blocking time. Definitely not something we want
/// parking the WebView main thread.
#[tauri::command]
pub async fn send_break(
    window: WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let sess = active_session(state.inner(), window.label()).ok_or("not connected")?;
    sess.send_break(serial::session::BREAK_DURATION)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn active_profile_id(
    window: WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> String {
    state.with_session(window.label(), |handle| handle.profile_id.clone())
}

#[tauri::command]
pub fn get_control_lines(
    window: WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlLines, String> {
    let sess = active_session(state.inner(), window.label()).ok_or("not connected")?;
    Ok(sess.control_lines())
}

// --- Internals ---------------------------------------------------------

fn active_session(state: &Arc<AppState>, label: &str) -> Option<Arc<Session>> {
    state.with_session(label, |handle| handle.session.clone())
}

/// Open a session for `profile`, wire data / exit callbacks, and
/// install the Session in shared state. Shared between the initial
/// Connect command and the auto-reconnect loop so both paths produce
/// identical sessions.
fn open_session(
    window: &WebviewWindow,
    state: &Arc<AppState>,
    profile: Profile,
) -> Result<(), serial::SessionError> {
    let label = window.label().to_string();
    if state.with_session(&label, |handle| handle.session.is_some()) {
        return Err(serial::SessionError::Io(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "already connected — disconnect first",
        )));
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

    let app = window.app_handle().clone();
    // Carry the emit-target label in a shared cell so a future
    // `migrate_session` can reroute the on_read / on_exit fan-out
    // without re-constructing Session.
    let label_cell = Arc::new(RwLock::new(label.clone()));
    let on_read = build_on_read(app.clone(), Arc::clone(&label_cell));
    let on_exit = build_on_exit(app, Arc::clone(state), Arc::clone(&label_cell));

    let sess = Arc::new(Session::open(cfg, on_read, on_exit)?);

    if profile.log_enabled {
        match open_session_log(state, &profile) {
            Ok(writer) => sess.set_log_writer(Some(writer)),
            Err(err) => log::error!("open session log: {}", err),
        }
    }

    state.with_session(&label, |handle| {
        handle.session = Some(Arc::clone(&sess));
        handle.profile_id = profile.id.clone();
        handle.profile_snapshot = Some(profile);
        handle.event_target_label = Some(label_cell);
    });
    Ok(())
}

/// Build the on-read callback that ferries serial bytes to the
/// originating window's renderer. AppHandle is passed in (rather
/// than WebviewWindow) so the callback is `Send + Sync` — the
/// read-pump thread that fires this is OS-owned, not Tauri's
/// runtime. The label arrives via `Arc<RwLock<String>>` so a
/// session-migration step can reroute the stream to a different
/// window after the fact (see `commands::window::migrate_session`).
fn build_on_read(app: AppHandle, label_cell: Arc<RwLock<String>>) -> OnRead {
    Arc::new(move |bytes: &[u8]| {
        let payload = base64::engine::general_purpose::STANDARD.encode(bytes);
        let label = label_cell.read().unwrap().clone();
        let _ = app.emit_to(label.as_str(), events::SERIAL_DATA, payload);
    })
}

fn build_on_exit(
    app: AppHandle,
    state: Arc<AppState>,
    label_cell: Arc<RwLock<String>>,
) -> OnExit {
    Arc::new(move |err: io::Error| {
        let label = label_cell.read().unwrap().clone();
        handle_session_exit(&app, &state, &label, err);
    })
}

/// Runs on the read-pump thread when the port returns an error. On
/// profiles that opt into auto-reconnect it kicks off a retry loop;
/// otherwise it clears session state and emits `serial:disconnect`.
fn handle_session_exit(
    app: &AppHandle,
    state: &Arc<AppState>,
    label: &str,
    err: io::Error,
) {
    let (old_session, profile) = state.with_session(label, |handle| {
        let sess = handle.session.take();
        let profile = handle.profile_snapshot.clone();
        (sess, profile)
    });

    // Close the orphaned session on a fresh thread — Session::close
    // joins the read-pump thread, and we're ON that thread right now.
    if let Some(sess) = old_session {
        thread::spawn(move || {
            let _ = sess.close();
        });
    }

    if let Some(profile) = profile {
        if profile.auto_reconnect {
            start_reconnect(app, state, label, profile);
            return;
        }
    }

    state.with_session(label, |handle| {
        handle.profile_id.clear();
        handle.profile_snapshot = None;
    });
    let _ = app.emit_to(label, events::SERIAL_DISCONNECT, err.to_string());
}

fn start_reconnect(
    app: &AppHandle,
    state: &Arc<AppState>,
    label: &str,
    profile: Profile,
) {
    let cancel = Arc::new(AtomicBool::new(false));
    state.with_session(label, |handle| {
        if let Some(prev) = handle.reconnect_cancel.take() {
            prev.store(true, Ordering::Release);
        }
        handle.reconnect_cancel = Some(Arc::clone(&cancel));
    });

    let _ = app.emit_to(label, events::SERIAL_RECONNECTING, profile.port_name.clone());

    let app = app.clone();
    let state = Arc::clone(state);
    let label = label.to_string();
    thread::Builder::new()
        .name("baudrun-reconnect".into())
        .spawn(move || {
            let deadline = Instant::now() + RECONNECT_TIMEOUT;
            loop {
                thread::sleep(RECONNECT_INTERVAL);
                if cancel.load(Ordering::Acquire) {
                    finish_failed_reconnect(&app, &state, &label, "reconnect cancelled");
                    return;
                }
                if Instant::now() >= deadline {
                    finish_failed_reconnect(&app, &state, &label, "reconnect timeout");
                    return;
                }
                // Re-derive the WebviewWindow from the AppHandle each
                // attempt — the user might've closed the window during
                // the reconnect loop, in which case we silently abort.
                let Some(window) = app.get_webview_window(&label) else {
                    finish_failed_reconnect(&app, &state, &label, "window closed");
                    return;
                };
                if open_session(&window, &state, profile.clone()).is_ok() {
                    let _ = app.emit_to(
                        label.as_str(),
                        events::SERIAL_RECONNECTED,
                        profile.id.clone(),
                    );
                    state.with_session(&label, |handle| {
                        handle.reconnect_cancel = None;
                    });
                    return;
                }
            }
        })
        .expect("spawn baudrun-reconnect thread");
}

fn finish_failed_reconnect(
    app: &AppHandle,
    state: &Arc<AppState>,
    label: &str,
    reason: &str,
) {
    state.with_session(label, |handle| {
        handle.profile_id.clear();
        handle.profile_snapshot = None;
        handle.reconnect_cancel = None;
    });
    let _ = app.emit_to(label, events::SERIAL_DISCONNECT, reason);
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
