//! File-transfer commands. `send_file` blocks until the transfer
//! completes, fails, or the caller calls `cancel_transfer`. Progress
//! flows out as `transfer:progress` events; completion / failure as
//! `transfer:complete` / `transfer:error`.

use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::events::{self, TransferProgress};
use crate::serial::Session;
use crate::state::AppState;
use crate::transfer::{self, ChannelReader, Options, XModemVariant};

/// Native file picker, returning "" on cancel so the frontend can
/// branch on empty string (matches the Go / Wails behaviour).
///
/// `async` is load-bearing: Tauri 2 dispatches sync commands onto the
/// runtime's main thread, where a `blocking_pick_file()` call would
/// deadlock the WebView event loop on Linux (WebKit2GTK) and Windows
/// (WebView2) — the dialog can't pump events because the thread that
/// drives it is parked. Marking the command `async` moves it onto the
/// async runtime, which has its own worker thread for the blocking
/// call while the main thread stays free to render the dialog.
#[tauri::command]
pub async fn pick_send_file(app: AppHandle) -> Result<String, String> {
    let picked = app
        .dialog()
        .file()
        .set_title("Choose a file to send")
        .blocking_pick_file();
    match picked {
        Some(p) => p
            .into_path()
            .map(|pb| pb.to_string_lossy().into_owned())
            .map_err(|e| format!("resolve path: {}", e)),
        None => Ok(String::new()),
    }
}

/// Drive an XMODEM/YMODEM transfer over the active session. Returns
/// when the transfer completes, fails, or is cancelled via
/// [`cancel_transfer`]. Emits `transfer:progress` after each ACKed
/// block and `transfer:complete` / `transfer:error` on the terminal
/// state.
#[tauri::command]
pub fn send_file(
    protocol: String,
    path: String,
    window: WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let label = window.label().to_string();
    // Acquire session + install transfer state under the session
    // lock so two send_file invocations from this window can't both
    // think they own the port. Two windows running parallel transfers
    // is fine — each has its own SessionHandle.
    let setup = state.with_session(&label, |handle| {
        if handle.transfer_cancel.is_some() {
            return Err("transfer already in progress".to_string());
        }
        let sess = handle
            .session
            .clone()
            .ok_or_else(|| "not connected".to_string())?;
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        handle.transfer_tx = Some(tx.clone());
        let cancel = Arc::new(AtomicBool::new(false));
        handle.transfer_cancel = Some(Arc::clone(&cancel));
        Ok((sess, tx, rx, cancel))
    });
    let (sess, tx, rx, cancel) = setup?;

    let app = window.app_handle().clone();
    let result = run_transfer(&app, &label, &sess, &protocol, &path, tx, rx, cancel);

    // Tear down transfer state regardless of outcome. Hold the
    // session lock only while mutating, not while emitting events.
    sess.end_transfer();
    state.with_session(&label, |handle| {
        handle.transfer_tx = None;
        handle.transfer_cancel = None;
    });

    let filename = Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    match result {
        Ok(()) => {
            let _ = app.emit_to(label.as_str(), events::TRANSFER_COMPLETE, filename);
            Ok(())
        }
        Err(msg) => {
            let _ = app.emit_to(label.as_str(), events::TRANSFER_ERROR, msg.clone());
            Err(msg)
        }
    }
}

/// Flip the cancel flag on an in-flight transfer. No-op when no
/// transfer is running in the calling window.
#[tauri::command]
pub fn cancel_transfer(window: WebviewWindow, state: State<'_, Arc<AppState>>) {
    let cancel = state.with_session(window.label(), |handle| handle.transfer_cancel.clone());
    if let Some(flag) = cancel {
        flag.store(true, Ordering::Release);
    }
}

// --- Internals ---------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn run_transfer(
    app: &AppHandle,
    label: &str,
    sess: &Arc<Session>,
    protocol: &str,
    path: &str,
    tx: mpsc::Sender<Vec<u8>>,
    rx: mpsc::Receiver<Vec<u8>>,
    cancel: Arc<AtomicBool>,
) -> Result<(), String> {
    let data = std::fs::read(path).map_err(|e| format!("read file: {}", e))?;

    // Bridge Session's incoming-bytes sink into the transfer
    // channel. `sess.end_transfer()` is called in send_file's
    // cleanup regardless of result, so this handler's lifetime is
    // bounded by the function.
    let sink = {
        let tx = tx;
        Arc::new(move |bytes: &[u8]| {
            let _ = tx.send(bytes.to_vec());
        })
    };
    sess.start_transfer(sink);

    let mut reader = ChannelReader::new(rx);
    let mut writer = SessionWriter {
        sess: Arc::clone(sess),
    };

    let progress_app = app.clone();
    let progress_label = label.to_string();
    let progress = Arc::new(move |sent: u64, total: u64| {
        let _ = progress_app.emit_to(
            progress_label.as_str(),
            events::TRANSFER_PROGRESS,
            TransferProgress { sent, total },
        );
    });
    let opts = Options {
        progress: Some(progress),
        cancel: Some(cancel),
    };

    let filename = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let result = match protocol {
        "xmodem" => transfer::send_xmodem(
            &mut reader,
            &mut writer,
            &data,
            XModemVariant::Classic,
            &opts,
        ),
        "xmodem-crc" => transfer::send_xmodem(
            &mut reader,
            &mut writer,
            &data,
            XModemVariant::Crc,
            &opts,
        ),
        "xmodem-1k" => transfer::send_xmodem(
            &mut reader,
            &mut writer,
            &data,
            XModemVariant::OneKilo,
            &opts,
        ),
        "ymodem" => transfer::send_ymodem(&mut reader, &mut writer, &filename, &data, &opts),
        other => return Err(format!("unknown protocol: {}", other)),
    };

    result.map_err(|e| e.to_string())
}

/// Adapter: Session::send returns a SessionError, but the transfer
/// module wants plain io::Write semantics.
struct SessionWriter {
    sess: Arc<Session>,
}

impl Write for SessionWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.sess
            .send(buf)
            .map(|_| buf.len())
            .map_err(session_error_to_io)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn session_error_to_io(err: crate::serial::SessionError) -> io::Error {
    match err {
        crate::serial::SessionError::Io(e) => e,
        other => io::Error::other(other),
    }
}
