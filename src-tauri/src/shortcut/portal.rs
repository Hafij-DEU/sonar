//! Global shortcuts through the XDG desktop portal (GNOME and KDE on Wayland).
//!
//! On Wayland an app cannot grab keys for itself. The desktop owns the grab and
//! tells Sonar over D-Bus when a shortcut goes down and up, so this needs no
//! input group, no root and no helper daemon.
//!
//! Each shortcut's portal id carries the key string the user picked
//! (`transcribe:super+alt+space`). A desktop remembers a shortcut by its id and
//! keeps the trigger it first stored, so a new key string must be a new id
//! for the desktop to offer the new keys.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut, Shortcut};
use futures_util::StreamExt;
use log::{error, info, warn};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;

use super::handler::handle_shortcut_event;
use super::tauri_impl;
use crate::settings::{self, ShortcutBinding};

/// The app id the desktop files Sonar's shortcuts under. It matches the
/// hidden `com.hafij.sonar.desktop` the package installs.
pub const APP_ID: &str = "com.hafij.sonar";

/// The bindings handed to the portal. The rest of Handy's bindings have no
/// Sonar UI and stay unbound.
const PORTAL_BINDINGS: [&str; 2] = ["transcribe", "transcribe_english"];

const STATUS_EVENT: &str = "portal-shortcuts-changed";

/// A press this soon after the last one, with no release between, is the
/// desktop repeating a held key. Key repeat runs every 25-100 ms, so a longer
/// gap means the release was lost and this is a new press.
const REPEAT_GAP: Duration = Duration::from_millis(1000);

/// Set when the portal could not be reached and the Tauri backend took over
/// for this run.
static FELL_BACK: AtomicBool = AtomicBool::new(false);

enum Command {
    /// Bind again if the wanted keys differ from the bound ones (`force`
    /// binds even when they match, to retry after the user declined).
    Rebind { force: bool },
    /// Ask the desktop to show its own shortcut editor.
    Configure,
}

#[derive(Default)]
struct Inner {
    /// Binding id -> key string the user picked in Sonar.
    wanted: HashMap<String, String>,
    /// What the live session was bound with.
    bound: HashMap<String, String>,
    /// Bindings paused while the UI records a new shortcut.
    suspended: HashSet<String>,
    /// Binding id -> trigger as the desktop describes it.
    triggers: HashMap<String, String>,
    /// Binding id -> when its last press event came, while the key is down.
    /// GNOME repeats the press event while a key is held.
    held: HashMap<String, Instant>,
    active: bool,
    error: Option<String>,
    tx: Option<mpsc::UnboundedSender<Command>>,
}

#[derive(Default)]
pub struct PortalShortcuts(Mutex<Inner>);

impl PortalShortcuts {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[derive(Serialize, Type, Clone, Debug)]
pub struct PortalShortcutTrigger {
    pub id: String,
    pub trigger: String,
}

#[derive(Serialize, Type, Clone, Debug)]
pub struct PortalShortcutsStatus {
    /// The portal backend is the selected implementation.
    pub enabled: bool,
    /// A session is bound and listening.
    pub active: bool,
    pub error: Option<String>,
    pub triggers: Vec<PortalShortcutTrigger>,
}

fn state(app: &AppHandle) -> &PortalShortcuts {
    if app.try_state::<PortalShortcuts>().is_none() {
        app.manage(PortalShortcuts::default());
    }
    app.state::<PortalShortcuts>().inner()
}

/// Start the portal session in the background. Safe to call again: a running
/// session is reused.
pub fn init_shortcuts(app: &AppHandle) {
    let st = state(app);
    let mut inner = st.lock();
    for (id, binding) in settings::get_bindings(app) {
        if PORTAL_BINDINGS.contains(&id.as_str()) {
            inner.wanted.insert(id, binding.current_binding);
        }
    }
    if let Some(tx) = &inner.tx {
        let _ = tx.send(Command::Rebind { force: false });
        return;
    }
    let (tx, rx) = mpsc::unbounded_channel();
    inner.tx = Some(tx);
    drop(inner);

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        run(app, rx).await;
    });
}

pub fn register_shortcut(app: &AppHandle, binding: ShortcutBinding) -> Result<(), String> {
    if FELL_BACK.load(Ordering::SeqCst) {
        return tauri_impl::register_shortcut(app, binding);
    }
    if !PORTAL_BINDINGS.contains(&binding.id.as_str()) {
        return Ok(());
    }
    let st = state(app);
    let mut inner = st.lock();
    inner.suspended.remove(&binding.id);
    let changed = inner.wanted.get(&binding.id) != Some(&binding.current_binding);
    inner.wanted.insert(binding.id.clone(), binding.current_binding);
    match &inner.tx {
        Some(tx) if changed => {
            let _ = tx.send(Command::Rebind { force: false });
        }
        Some(_) => {}
        None => {
            drop(inner);
            init_shortcuts(app);
        }
    }
    Ok(())
}

/// The desktop keeps the grab; Sonar just stops acting on the shortcut.
pub fn unregister_shortcut(app: &AppHandle, binding: ShortcutBinding) -> Result<(), String> {
    if FELL_BACK.load(Ordering::SeqCst) {
        return tauri_impl::unregister_shortcut(app, binding);
    }
    if PORTAL_BINDINGS.contains(&binding.id.as_str()) {
        state(app).lock().suspended.insert(binding.id);
    }
    Ok(())
}

/// Any non-empty key string can be offered; the desktop has the last word.
pub fn validate_shortcut(raw: &str) -> Result<(), String> {
    if to_portal_trigger(raw).is_some() {
        Ok(())
    } else {
        Err(format!("'{raw}' has no key besides modifiers"))
    }
}

/// `super+alt+space` -> `LOGO+ALT+space` (XDG shortcuts trigger syntax:
/// modifiers CTRL, ALT, SHIFT, NUM, LOGO, then an XKB key name).
pub fn to_portal_trigger(binding: &str) -> Option<String> {
    let mut mods: Vec<&str> = Vec::new();
    let mut key: Option<String> = None;
    for part in binding.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        let lower = part.to_ascii_lowercase();
        let modifier = match lower.as_str() {
            "ctrl" | "control" | "ctrl_l" | "ctrl_r" => Some("CTRL"),
            "alt" | "option" | "alt_l" | "alt_r" | "altgr" => Some("ALT"),
            "shift" | "shift_l" | "shift_r" => Some("SHIFT"),
            "super" | "meta" | "cmd" | "command" | "win" | "logo" | "super_l" | "super_r" => {
                Some("LOGO")
            }
            _ => None,
        };
        match modifier {
            Some(m) if !mods.contains(&m) => mods.push(m),
            Some(_) => {}
            None => key = Some(xkb_key_name(&lower)),
        }
    }
    let key = key?;
    let mut out = mods.join("+");
    if !out.is_empty() {
        out.push('+');
    }
    out.push_str(&key);
    Some(out)
}

fn xkb_key_name(key: &str) -> String {
    let named = match key {
        "space" => "space",
        "enter" | "return" => "Return",
        "tab" => "Tab",
        "esc" | "escape" => "Escape",
        "backspace" => "BackSpace",
        "delete" | "del" => "Delete",
        "insert" | "ins" => "Insert",
        "home" => "Home",
        "end" => "End",
        "pageup" | "page_up" => "Page_Up",
        "pagedown" | "page_down" => "Page_Down",
        "up" | "arrowup" => "Up",
        "down" | "arrowdown" => "Down",
        "left" | "arrowleft" => "Left",
        "right" | "arrowright" => "Right",
        "pause" => "Pause",
        "capslock" => "Caps_Lock",
        "printscreen" | "print" => "Print",
        "scrolllock" => "Scroll_Lock",
        "minus" | "-" => "minus",
        "equal" | "=" => "equal",
        "comma" | "," => "comma",
        "period" | "." => "period",
        "slash" | "/" => "slash",
        "backslash" | "\\" => "backslash",
        "semicolon" | ";" => "semicolon",
        "quote" | "'" => "apostrophe",
        "backquote" | "`" => "grave",
        "bracketleft" | "[" => "bracketleft",
        "bracketright" | "]" => "bracketright",
        _ => "",
    };
    if !named.is_empty() {
        return named.to_string();
    }
    // F1..F24
    if let Some(n) = key.strip_prefix('f') {
        if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) {
            return format!("F{n}");
        }
    }
    // keya / digit1 style names from the browser capture
    if let Some(c) = key.strip_prefix("key").filter(|c| c.len() == 1) {
        return c.to_string();
    }
    if let Some(d) = key.strip_prefix("digit").filter(|d| d.len() == 1) {
        return d.to_string();
    }
    key.to_string()
}

fn description(id: &str) -> &'static str {
    match id {
        "transcribe" => "Dictate in Bangla",
        "transcribe_english" => "Dictate in English",
        _ => "Sonar shortcut",
    }
}

fn portal_id(id: &str, binding: &str) -> String {
    format!("{id}:{binding}")
}

/// `transcribe:super+alt+space` -> (`transcribe`, `super+alt+space`).
fn split_portal_id(portal_id: &str) -> (&str, &str) {
    portal_id.split_once(':').unwrap_or((portal_id, ""))
}

pub fn status(app: &AppHandle) -> PortalShortcutsStatus {
    let enabled = settings::get_settings(app).keyboard_implementation
        == settings::KeyboardImplementation::Portal;
    let inner = state(app).lock();
    let mut triggers: Vec<PortalShortcutTrigger> = inner
        .triggers
        .iter()
        .map(|(id, trigger)| PortalShortcutTrigger {
            id: id.clone(),
            trigger: trigger.clone(),
        })
        .collect();
    triggers.sort_by(|a, b| a.id.cmp(&b.id));
    PortalShortcutsStatus {
        enabled,
        active: inner.active,
        error: inner.error.clone(),
        triggers,
    }
}

fn publish(app: &AppHandle) {
    let _ = app.emit(STATUS_EVENT, status(app));
}

fn set_error(app: &AppHandle, message: String) {
    {
        let mut inner = state(app).lock();
        inner.active = false;
        inner.error = Some(message);
    }
    publish(app);
}

fn record_triggers(app: &AppHandle, shortcuts: &[Shortcut]) {
    let mut inner = state(app).lock();
    for shortcut in shortcuts {
        let (id, _) = split_portal_id(shortcut.id());
        inner
            .triggers
            .insert(id.to_string(), shortcut.trigger_description().to_string());
    }
}

/// Hand the shortcuts to the Tauri backend for this run.
fn fall_back(app: &AppHandle, reason: String) {
    error!("Portal shortcuts unavailable ({reason}); using the Tauri backend for this run");
    set_error(app, reason);
    FELL_BACK.store(true, Ordering::SeqCst);
    tauri_impl::init_shortcuts(app);
    crate::portal_input::prepare(app);
}

enum Event {
    Pressed(String),
    Released(String),
    Changed(Vec<Shortcut>),
}

async fn run(app: AppHandle, mut rx: mpsc::UnboundedReceiver<Command>) {
    match ashpd::AppID::try_from(APP_ID) {
        Ok(app_id) => {
            if let Err(e) = ashpd::register_host_app(app_id).await {
                // Older portals have no registry; the desktop then names the
                // app from its process instead.
                warn!("Could not register Sonar's app id with the portal: {e}");
            }
        }
        Err(e) => warn!("Invalid app id {APP_ID}: {e}"),
    }

    let proxy = match GlobalShortcuts::new().await {
        Ok(proxy) => proxy,
        Err(e) => return fall_back(&app, format!("no GlobalShortcuts portal: {e}")),
    };

    let streams = async {
        Ok::<_, ashpd::Error>((
            proxy.receive_activated().await?,
            proxy.receive_deactivated().await?,
            proxy.receive_shortcuts_changed().await?,
        ))
    }
    .await;
    let (activated, deactivated, changed) = match streams {
        Ok(s) => s,
        Err(e) => return fall_back(&app, format!("portal signals failed: {e}")),
    };
    let mut events = Box::pin(futures_util::stream::select(
        activated.map(|a| Event::Pressed(a.shortcut_id().to_string())),
        futures_util::stream::select(
            deactivated.map(|d| Event::Released(d.shortcut_id().to_string())),
            changed.map(|c| Event::Changed(c.shortcuts().to_vec())),
        ),
    ));

    'session: loop {
        let session = match proxy.create_session().await {
            Ok(session) => session,
            Err(e) => return fall_back(&app, format!("could not open a portal session: {e}")),
        };

        let wanted = state(&app).lock().wanted.clone();
        let shortcuts: Vec<NewShortcut> = PORTAL_BINDINGS
            .iter()
            .filter_map(|id| {
                let binding = wanted.get(*id)?;
                Some(
                    NewShortcut::new(portal_id(id, binding), description(id))
                        .preferred_trigger(to_portal_trigger(binding).as_deref()),
                )
            })
            .collect();

        info!("Binding {} portal shortcut(s)", shortcuts.len());
        let bound = match proxy.bind_shortcuts(&session, &shortcuts, None).await {
            Ok(request) => request.response(),
            Err(e) => Err(e),
        };
        match bound {
            Ok(result) => {
                {
                    let mut inner = state(&app).lock();
                    inner.triggers.clear();
                    inner.held.clear();
                    inner.bound = wanted;
                    inner.active = true;
                    inner.error = None;
                }
                record_triggers(&app, result.shortcuts());
                for s in result.shortcuts() {
                    info!("Portal shortcut {} -> {}", s.id(), s.trigger_description());
                }
            }
            Err(e) => {
                warn!("Portal shortcut binding failed: {e}");
                {
                    let mut inner = state(&app).lock();
                    inner.bound = wanted;
                }
                set_error(&app, format!("The desktop did not accept the shortcuts ({e})."));
            }
        }
        publish(&app);
        // After the shortcuts dialog, ask for typing access, so both prompts
        // come at startup instead of the first paste.
        crate::portal_input::prepare(&app);

        loop {
            let next_command = std::future::poll_fn(|cx| rx.poll_recv(cx));
            let next_event = events.next();
            futures_util::pin_mut!(next_command);
            match futures_util::future::select(next_event, next_command).await {
                futures_util::future::Either::Left((Some(event), _)) => {
                    handle_event(&app, event);
                }
                futures_util::future::Either::Left((None, _)) => {
                    return fall_back(&app, "the portal connection closed".to_string());
                }
                futures_util::future::Either::Right((None, _)) => return,
                futures_util::future::Either::Right((Some(command), _)) => {
                    let mut force = false;
                    let mut configure = false;
                    let mut pending = Some(command);
                    // Changing one binding sends several commands at once.
                    while let Some(command) = pending.take().or_else(|| rx.try_recv().ok()) {
                        match command {
                            Command::Rebind { force: f } => force |= f,
                            Command::Configure => configure = true,
                        }
                    }
                    if configure {
                        if let Err(e) = proxy.configure_shortcuts(&session, None, None).await {
                            warn!("Portal shortcut editor unavailable: {e}");
                            force = true;
                        }
                    }
                    let differs = {
                        let inner = state(&app).lock();
                        inner.wanted != inner.bound
                    };
                    if force || differs {
                        let _ = session.close().await;
                        continue 'session;
                    }
                }
            }
        }
    }
}

fn handle_event(app: &AppHandle, event: Event) {
    let (portal_id, pressed) = match event {
        Event::Pressed(id) => (id, true),
        Event::Released(id) => (id, false),
        Event::Changed(shortcuts) => {
            record_triggers(app, &shortcuts);
            publish(app);
            return;
        }
    };
    let (id, binding) = split_portal_id(&portal_id);
    {
        let mut inner = state(app).lock();
        // Only the live session's ids count, and none while the UI records keys.
        if inner.suspended.contains(id) || inner.bound.get(id).map(String::as_str) != Some(binding)
        {
            inner.held.remove(id);
            return;
        }
        if !is_new_edge(&mut inner.held, id, pressed, Instant::now()) {
            return;
        }
    }
    handle_shortcut_event(app, id, binding, pressed);
}

/// Turns the desktop's events into one press and one release per key stroke.
fn is_new_edge(
    held: &mut HashMap<String, Instant>,
    id: &str,
    pressed: bool,
    now: Instant,
) -> bool {
    if pressed {
        let repeat = held
            .get(id)
            .is_some_and(|last| now.duration_since(*last) < REPEAT_GAP);
        held.insert(id.to_string(), now);
        !repeat
    } else {
        held.remove(id).is_some()
    }
}

/// Bind again, even with unchanged keys (after the user declined).
pub fn retry(app: &AppHandle) {
    if FELL_BACK.load(Ordering::SeqCst) {
        return;
    }
    match &state(app).lock().tx {
        Some(tx) => {
            let _ = tx.send(Command::Rebind { force: true });
            return;
        }
        None => {}
    }
    init_shortcuts(app);
}

pub fn configure(app: &AppHandle) {
    if let Some(tx) = &state(app).lock().tx {
        let _ = tx.send(Command::Configure);
    }
}

#[tauri::command]
#[specta::specta]
pub fn get_portal_shortcuts_status(app: AppHandle) -> PortalShortcutsStatus {
    status(&app)
}

/// Ask the desktop again for the shortcuts, or open its shortcut editor.
#[tauri::command]
#[specta::specta]
pub fn setup_portal_shortcuts(app: AppHandle, open_editor: bool) {
    if open_editor {
        configure(&app);
    } else {
        retry(&app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_key_repeats_count_once() {
        let mut held = HashMap::new();
        let t = Instant::now();
        let ms = Duration::from_millis;
        assert!(is_new_edge(&mut held, "a", true, t));
        // Key repeat while held.
        assert!(!is_new_edge(&mut held, "a", true, t + ms(500)));
        assert!(!is_new_edge(&mut held, "a", true, t + ms(1400)));
        assert!(is_new_edge(&mut held, "a", false, t + ms(1500)));
        // A stray release counts nothing.
        assert!(!is_new_edge(&mut held, "a", false, t + ms(1600)));
        assert!(is_new_edge(&mut held, "a", true, t + ms(1700)));
        // Release lost: a press long after the last one is new.
        assert!(is_new_edge(&mut held, "a", true, t + ms(5000)));
        // Other bindings are separate.
        assert!(is_new_edge(&mut held, "b", true, t + ms(5000)));
    }

    #[test]
    fn triggers_use_portal_syntax() {
        assert_eq!(
            to_portal_trigger("super+alt+space").as_deref(),
            Some("LOGO+ALT+space")
        );
        assert_eq!(
            to_portal_trigger("super+alt+shift+space").as_deref(),
            Some("LOGO+ALT+SHIFT+space")
        );
        assert_eq!(to_portal_trigger("ctrl+f9").as_deref(), Some("CTRL+F9"));
        assert_eq!(to_portal_trigger("ctrl+shift+keyd").as_deref(), Some("CTRL+SHIFT+d"));
        assert_eq!(to_portal_trigger("super+alt"), None);
    }

    #[test]
    fn portal_ids_round_trip() {
        let id = portal_id("transcribe_english", "super+alt+shift+space");
        assert_eq!(
            split_portal_id(&id),
            ("transcribe_english", "super+alt+shift+space")
        );
    }
}
