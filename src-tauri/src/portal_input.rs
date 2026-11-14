//! Key presses through the XDG RemoteDesktop portal.
//!
//! GNOME's compositor accepts no virtual keyboards from apps, and tools such
//! as ydotool need root or the input group. The RemoteDesktop portal lets the
//! user allow keyboard input once; the restore token kept in Sonar's data
//! folder skips the prompt on later runs until the user revokes it.

use std::sync::mpsc as std_mpsc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use ashpd::desktop::remote_desktop::{DeviceType, KeyState, RemoteDesktop};
use ashpd::desktop::{PersistMode, Session};
use log::{info, warn};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::settings::PasteMethod;

const TOKEN_FILE: &str = "remote-desktop-token";

// XKB keysyms
const KEY_CONTROL_L: i32 = 0xffe3;
const KEY_SHIFT_L: i32 = 0xffe1;
const KEY_V: i32 = 0x0076;
const KEY_INSERT: i32 = 0xff63;

struct Live {
    proxy: RemoteDesktop<'static>,
    session: Session<'static, RemoteDesktop<'static>>,
}

static LIVE: Mutex<Option<Live>> = Mutex::const_new(None);
static PREPARED: AtomicBool = AtomicBool::new(false);

/// How long a paste waits for keyboard access, including the first-run dialog.
const PASTE_WAIT: Duration = Duration::from_secs(120);

fn token_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join(TOKEN_FILE))
}

async fn open(app: &AppHandle) -> Result<Live, String> {
    let proxy = RemoteDesktop::new()
        .await
        .map_err(|e| format!("no RemoteDesktop portal: {e}"))?;
    let session = proxy
        .create_session()
        .await
        .map_err(|e| format!("could not open a RemoteDesktop session: {e}"))?;

    let token = token_path(app).and_then(|p| std::fs::read_to_string(p).ok());
    let token = token.as_deref().map(str::trim).filter(|t| !t.is_empty());
    proxy
        .select_devices(
            &session,
            DeviceType::Keyboard.into(),
            token,
            PersistMode::ExplicitlyRevoked,
        )
        .await
        .and_then(|r| r.response())
        .map_err(|e| format!("keyboard access was not granted: {e}"))?;

    let started = proxy
        .start(&session, None)
        .await
        .and_then(|r| r.response())
        .map_err(|e| format!("keyboard access was not granted: {e}"))?;

    if !started.devices().contains(DeviceType::Keyboard) {
        return Err("the desktop gave no keyboard access".to_string());
    }
    if let (Some(new_token), Some(path)) = (started.restore_token(), token_path(app)) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&path, new_token) {
            warn!("Could not save the RemoteDesktop restore token: {e}");
        }
    }
    info!("RemoteDesktop keyboard session started");
    Ok(Live { proxy, session })
}

async fn tap(live: &Live, modifiers: &[i32], key: i32) -> Result<(), ashpd::Error> {
    for m in modifiers {
        live.proxy
            .notify_keyboard_keysym(&live.session, *m, KeyState::Pressed)
            .await?;
    }
    live.proxy
        .notify_keyboard_keysym(&live.session, key, KeyState::Pressed)
        .await?;
    live.proxy
        .notify_keyboard_keysym(&live.session, key, KeyState::Released)
        .await?;
    for m in modifiers.iter().rev() {
        live.proxy
            .notify_keyboard_keysym(&live.session, *m, KeyState::Released)
            .await?;
    }
    Ok(())
}

async fn send_combo(
    app: &AppHandle,
    modifiers: &[i32],
    key: i32,
    deadline: Instant,
) -> Result<(), String> {
    let mut live = LIVE.lock().await;
    // A session can end under us (revoked, or the portal restarted): reopen once.
    for attempt in 0..2 {
        if live.is_none() {
            *live = Some(open(app).await?);
        }
        // The caller gave up waiting: a late paste would land in whatever
        // window has focus by now.
        if Instant::now() > deadline {
            return Err("keyboard access came too late for this paste".to_string());
        }
        let session = live.as_ref().expect("session opened above");
        match tap(session, modifiers, key).await {
            Ok(()) => return Ok(()),
            Err(e) if attempt == 0 => {
                warn!("RemoteDesktop session failed ({e}); reopening");
                if let Some(old) = live.take() {
                    let _ = old.session.close().await;
                }
            }
            Err(e) => return Err(format!("could not send keys: {e}")),
        }
    }
    unreachable!("the loop returns on its second attempt")
}

/// Asks for keyboard access once, at startup, so the permission dialog
/// does not hold up the first paste. Does nothing on later calls.
pub fn prepare(app: &AppHandle) {
    if PREPARED.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut live = LIVE.lock().await;
        if live.is_none() {
            match open(&app).await {
                Ok(session) => *live = Some(session),
                Err(e) => warn!("Keyboard access not ready at startup: {e}"),
            }
        }
    });
}

/// Sends the paste key combination through the portal. Blocks the calling
/// worker thread; the first call may wait for the user to allow access.
pub fn send_paste_keys(app: &AppHandle, paste_method: &PasteMethod) -> Result<(), String> {
    let (modifiers, key): (&'static [i32], i32) = match paste_method {
        PasteMethod::CtrlV | PasteMethod::Direct => (&[KEY_CONTROL_L], KEY_V),
        PasteMethod::CtrlShiftV => (&[KEY_CONTROL_L, KEY_SHIFT_L], KEY_V),
        PasteMethod::ShiftInsert => (&[KEY_SHIFT_L], KEY_INSERT),
        _ => return Err("Unsupported paste method".into()),
    };
    let (tx, rx) = std_mpsc::sync_channel(1);
    let app = app.clone();
    let deadline = Instant::now() + PASTE_WAIT;
    tauri::async_runtime::spawn(async move {
        let _ = tx.send(send_combo(&app, modifiers, key, deadline).await);
    });
    // Two minutes leaves time to answer the first-run permission dialog.
    rx.recv_timeout(PASTE_WAIT)
        .map_err(|_| "the RemoteDesktop portal did not answer".to_string())?
}
