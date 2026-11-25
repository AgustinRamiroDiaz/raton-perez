//! Writing-tools popup: shows a transcript and runs LLM prompts over it.

use crate::actions::{post_process_with_prompt, PostProcessOutcome, POST_PROCESS_TIMEOUT};
use crate::settings::get_settings;
use log::error;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const LABEL: &str = "popup";

// The window may still be loading when a transcript arrives, so the page
// reads the latest one on mount as well as from the event.
static LATEST_TEXT: Mutex<String> = Mutex::new(String::new());

/// The app that was in front when the popup opened, so Paste can return to it.
#[cfg(target_os = "macos")]
static PREVIOUS_APP_PID: Mutex<Option<i32>> = Mutex::new(None);

/// Where the user was typing when the hotkey was pressed.
#[cfg(any(target_os = "macos", target_os = "linux"))]
static ANCHOR: Mutex<Option<crate::caret::Anchor>> = Mutex::new(None);

/// Card size in logical pixels. Keep in sync with the layout in Popup.tsx.
#[cfg(any(target_os = "macos", target_os = "linux"))]
const CARD_SIZE: (f64, f64) = (420.0, 360.0);
/// Gap between the caret and the card, in logical pixels.
#[cfg(any(target_os = "macos", target_os = "linux"))]
const CARD_GAP: f64 = 8.0;

#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    panel!(PopupPanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false,
            is_floating_panel: true
        }
    })
}

/// Called when the writing-tools hotkey starts a recording: notes the app and
/// caret the user is typing in, before anything else can move focus.
pub fn prepare() {
    #[cfg(target_os = "macos")]
    std::thread::spawn(|| {
        remember_frontmost_app();
        let pid = *PREVIOUS_APP_PID.lock().unwrap();
        *ANCHOR.lock().unwrap() = crate::caret::locate(pid);
    });

    #[cfg(target_os = "linux")]
    std::thread::spawn(|| {
        crate::caret::enable_accessibility();
        *ANCHOR.lock().unwrap() = crate::caret::locate();
    });
}

pub fn show(app: &AppHandle, text: String) {
    *LATEST_TEXT.lock().unwrap() = text.clone();

    #[cfg(target_os = "macos")]
    {
        remember_frontmost_app();
        let anchor = ANCHOR.lock().unwrap().take().or_else(|| {
            let pid = *PREVIOUS_APP_PID.lock().unwrap();
            crate::caret::locate(pid)
        });
        // Panels must be created, moved and shown on the main thread.
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            show_panel(&handle, anchor);
            let _ = handle.emit_to(LABEL, "popup-text", text);
        });
    }

    #[cfg(target_os = "linux")]
    {
        let anchor = ANCHOR.lock().unwrap().take();
        // GTK must be touched on the main thread.
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            linux::show_card(&handle, anchor);
            let _ = handle.emit_to(LABEL, "popup-text", text);
        });
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let window = match app.get_webview_window(LABEL) {
            Some(w) => w,
            None => {
                let mut builder = tauri::WebviewWindowBuilder::new(
                    app,
                    LABEL,
                    tauri::WebviewUrl::App("src/popup/index.html".into()),
                )
                .title("Ratón Pérez")
                .inner_size(600.0, 640.0)
                .always_on_top(true)
                .center();
                if let Some(data_dir) = crate::portable::data_dir() {
                    builder = builder.data_directory(data_dir.join("webview"));
                }
                match builder.build() {
                    Ok(w) => w,
                    Err(e) => {
                        error!("Failed to create popup window: {e}");
                        return;
                    }
                }
            }
        };
        // show() leaves a minimized window minimized.
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit_to(LABEL, "popup-text", text);
    }
}

/// Opens the popup as a card beside the caret. macOS won't let a background
/// app activate itself from a global hotkey, so the card is a non-activating
/// panel: it takes keyboard focus while the app the user is typing in stays
/// active, the way Spotlight and Grammarly do.
#[cfg(target_os = "macos")]
fn show_panel(app: &AppHandle, anchor: Option<crate::caret::Anchor>) {
    use tauri_nspanel::{CollectionBehavior, ManagerExt, PanelBuilder, PanelLevel, StyleMask};

    let panel = match app.get_webview_panel(LABEL) {
        Ok(panel) => panel,
        Err(_) => {
            let data_dir = crate::portable::data_dir();
            let built = PanelBuilder::<_, PopupPanel>::new(app, LABEL)
                .url(tauri::WebviewUrl::App("src/popup/index.html".into()))
                .title("Ratón Pérez")
                .size(tauri::Size::Logical(tauri::LogicalSize {
                    width: CARD_SIZE.0,
                    height: CARD_SIZE.1,
                }))
                .level(PanelLevel::Floating)
                .hides_on_deactivate(false)
                .has_shadow(true)
                .transparent(true)
                .style_mask(StyleMask::empty().borderless().nonactivating_panel())
                .collection_behavior(
                    CollectionBehavior::new()
                        .move_to_active_space()
                        .full_screen_auxiliary(),
                )
                .with_window(move |w| {
                    let w = w.decorations(false).transparent(true);
                    match data_dir {
                        Some(dir) => w.data_directory(dir.join("webview")),
                        None => w,
                    }
                })
                .build();
            match built {
                Ok(panel) => panel,
                Err(e) => {
                    error!("Failed to create popup panel: {e}");
                    return;
                }
            }
        }
    };
    if let Some(window) = app.get_webview_window(LABEL) {
        place_near(&window, anchor);
    }
    panel.show_and_make_key();
}

/// Moves the card just below the caret, or above it when there's no room
/// below, keeping it on the caret's screen. Without a caret it goes by the
/// mouse pointer. Call on the main thread.
#[cfg(target_os = "macos")]
fn place_near(window: &tauri::WebviewWindow, anchor: Option<crate::caret::Anchor>) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSEvent, NSScreen, NSWindow};
    use objc2_foundation::NSPoint;

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Ok(ptr) = window.ns_window() else {
        return;
    };
    // SAFETY: `ns_window` is the live NSWindow behind `window`, and this runs
    // on the main thread.
    let ns_window = unsafe { &*(ptr as *const NSWindow) };

    let screens = NSScreen::screens(mtm);
    let Some(primary) = screens.firstObject() else {
        return;
    };
    // Accessibility uses a top-left origin; AppKit a bottom-left one.
    let primary_height = primary.frame().size.height;
    let anchor = anchor.unwrap_or_else(|| {
        let mouse = NSEvent::mouseLocation();
        crate::caret::Anchor {
            x: mouse.x,
            y: primary_height - mouse.y,
            width: 0.0,
            height: 0.0,
        }
    });
    let caret_top = primary_height - anchor.y;
    let caret_bottom = primary_height - (anchor.y + anchor.height);

    let contains = |s: &NSScreen| {
        let f = s.frame();
        anchor.x >= f.origin.x
            && anchor.x <= f.origin.x + f.size.width
            && caret_bottom >= f.origin.y
            && caret_bottom <= f.origin.y + f.size.height
    };
    let screen = screens.iter().find(|s| contains(s)).unwrap_or(primary);
    let area = screen.visibleFrame();
    let size = ns_window.frame().size;

    let mut top = caret_bottom - CARD_GAP;
    if top - size.height < area.origin.y {
        top = caret_top + CARD_GAP + size.height;
    }
    let top = top
        .min(area.origin.y + area.size.height)
        .max(area.origin.y + size.height);
    let left = anchor
        .x
        .min(area.origin.x + area.size.width - size.width)
        .max(area.origin.x);
    ns_window.setFrameTopLeftPoint(NSPoint::new(left, top));
}

/// Linux: the same card beside the caret. On X11 it's a borderless window
/// placed at the caret and given focus. On Wayland, where apps can't place
/// windows or learn where another app's caret is, it's a layer-shell surface
/// centered on screen that takes the keyboard while it's open.
#[cfg(target_os = "linux")]
mod linux {
    use super::{CARD_GAP, CARD_SIZE, LABEL};
    use crate::caret::Anchor;
    use crate::utils;
    use log::{debug, error};
    use tauri::{AppHandle, Manager, PhysicalPosition, WebviewWindow};

    pub(super) fn show_card(app: &AppHandle, anchor: Option<Anchor>) {
        let window = match app.get_webview_window(LABEL) {
            Some(w) => w,
            None => match build(app) {
                Some(w) => w,
                None => return,
            },
        };
        let layer = is_layer(&window);
        if !layer {
            place_near(app, &window, anchor);
        }
        let _ = window.show();
        if !layer {
            focus(&window);
        }
    }

    fn build(app: &AppHandle) -> Option<WebviewWindow> {
        let mut builder = tauri::WebviewWindowBuilder::new(
            app,
            LABEL,
            tauri::WebviewUrl::App("src/popup/index.html".into()),
        )
        .title("Ratón Pérez")
        .inner_size(CARD_SIZE.0, CARD_SIZE.1)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false);
        if let Some(data_dir) = crate::portable::data_dir() {
            builder = builder.data_directory(data_dir.join("webview"));
        }
        let window = match builder.build() {
            Ok(w) => w,
            Err(e) => {
                error!("Failed to create popup window: {e}");
                return None;
            }
        };
        if utils::is_wayland() && init_layer_shell(&window) {
            debug!("Popup uses GTK layer shell");
        }
        Some(window)
    }

    fn init_layer_shell(window: &WebviewWindow) -> bool {
        use gtk::prelude::{GtkWindowExt, WidgetExt};
        use gtk_layer_shell::{KeyboardMode, Layer, LayerShell};

        if utils::env_flag_enabled("HANDY_NO_GTK_LAYER_SHELL") || !gtk_layer_shell::is_supported() {
            return false;
        }
        let Ok(gtk_window) = window.gtk_window() else {
            return false;
        };
        gtk_window.init_layer_shell();
        gtk_window.set_layer(Layer::Overlay);
        // Takes the keyboard while shown; it goes back to the app the user was
        // typing in when the card hides.
        gtk_window.set_keyboard_mode(KeyboardMode::Exclusive);
        gtk_window.set_exclusive_zone(0);
        // No anchors: the compositor centers the surface. Layer surfaces take
        // their size from the size request (see overlay.rs).
        gtk_window.set_size_request(CARD_SIZE.0 as i32, CARD_SIZE.1 as i32);
        gtk_window.resize(1, 1);
        gtk_window.is_layer_window()
    }

    fn is_layer(window: &WebviewWindow) -> bool {
        use gtk_layer_shell::LayerShell;
        window.gtk_window().is_ok_and(|w| w.is_layer_window())
    }

    /// Moves the card just below the caret, or above it when there's no room
    /// below, keeping it on the caret's monitor. Without a caret it goes by the
    /// mouse pointer.
    fn place_near(app: &AppHandle, window: &WebviewWindow, anchor: Option<Anchor>) {
        let anchor = anchor.or_else(|| {
            let p = app.cursor_position().ok()?;
            Some(Anchor {
                x: p.x,
                y: p.y,
                width: 0.0,
                height: 0.0,
            })
        });
        let Some(anchor) = anchor else {
            let _ = window.center();
            return;
        };
        let Ok(Some(monitor)) = app.monitor_from_point(anchor.x, anchor.y) else {
            let _ = window.center();
            return;
        };
        let scale = monitor.scale_factor();
        let area = monitor.work_area();
        let (left_edge, top_edge) = (f64::from(area.position.x), f64::from(area.position.y));
        let (right_edge, bottom_edge) = (
            left_edge + f64::from(area.size.width),
            top_edge + f64::from(area.size.height),
        );
        let (w, h) = (CARD_SIZE.0 * scale, CARD_SIZE.1 * scale);
        let gap = CARD_GAP * scale;

        let mut top = anchor.y + anchor.height + gap;
        if top + h > bottom_edge {
            top = anchor.y - gap - h;
        }
        let top = top.min(bottom_edge - h).max(top_edge);
        let left = anchor.x.min(right_edge - w).max(left_edge);
        let _ = window.set_position(PhysicalPosition::new(
            left.round() as i32,
            top.round() as i32,
        ));
    }

    /// X11 window managers refuse focus to a window raised without a recent
    /// user timestamp; a global hotkey gives us none, so use the server's.
    fn focus(window: &WebviewWindow) {
        use gtk::glib::Cast;
        use gtk::prelude::{GtkWindowExt, WidgetExt};

        let _ = window.set_focus();
        let Ok(gtk_window) = window.gtk_window() else {
            return;
        };
        let timestamp = gtk_window
            .window()
            .and_then(|w| w.downcast::<gdkx11::X11Window>().ok())
            .map(|w| gdkx11::functions::x11_get_server_time(&w));
        if let Some(timestamp) = timestamp {
            gtk_window.present_with_time(timestamp);
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn popup_get_text() -> String {
    LATEST_TEXT.lock().unwrap().clone()
}

/// Runs a saved prompt (`prompt_id`) or a free-form instruction (`custom`) over `text`.
#[tauri::command]
#[specta::specta]
pub async fn popup_run_prompt(
    app: AppHandle,
    text: String,
    prompt_id: Option<String>,
    custom: Option<String>,
) -> Result<String, String> {
    let settings = get_settings(&app);
    let prompt = match (prompt_id, custom) {
        (_, Some(instruction)) if !instruction.trim().is_empty() => format!(
            "<text>\n${{output}}\n</text>\n\nApply this change to the text: {instruction}\n\
             Output only the changed text, with no preamble."
        ),
        (Some(id), _) => settings
            .post_process_prompts
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.prompt.clone())
            .ok_or_else(|| format!("Prompt '{id}' not found"))?,
        _ => return Err("No prompt given".into()),
    };
    match tokio::time::timeout(
        POST_PROCESS_TIMEOUT,
        post_process_with_prompt(&settings, &text, &prompt),
    )
    .await
    {
        Ok(PostProcessOutcome::Done(out)) => Ok(out.trim().to_string()),
        Ok(PostProcessOutcome::Skipped) => Ok(text),
        Ok(PostProcessOutcome::Failed) => {
            Err("The AI request failed. Check the post-processing provider, model and API key in settings.".into())
        }
        Err(_) => Err("The AI request timed out.".into()),
    }
}

#[cfg(target_os = "macos")]
fn remember_frontmost_app() {
    use objc2_app_kit::NSWorkspace;
    let Some(front) = NSWorkspace::sharedWorkspace().frontmostApplication() else {
        return;
    };
    let pid = front.processIdentifier();
    // When the popup is already in front, keep the app recorded the first time.
    if pid != std::process::id() as i32 {
        *PREVIOUS_APP_PID.lock().unwrap() = Some(pid);
    }
}

/// Hiding a window doesn't deactivate the app on macOS, so the paste keystroke
/// would land back in Ratón Pérez. Bring the app the user came from to the front.
#[cfg(target_os = "macos")]
fn activate_previous_app() {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
    let Some(pid) = *PREVIOUS_APP_PID.lock().unwrap() else {
        return;
    };
    if let Some(prev) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
        #[allow(deprecated)]
        prev.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps);
    }
}

/// Hides the popup, returns focus to the previous app, then pastes `text` there.
#[tauri::command]
#[specta::specta]
pub fn popup_paste(app: AppHandle, text: String) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.hide();
    }
    #[cfg(target_os = "macos")]
    activate_previous_app();
    std::thread::spawn(move || {
        // shortcut: fixed wait for the WM to refocus the previous window; tune if pastes land in the wrong place.
        std::thread::sleep(Duration::from_millis(250));
        if let Err(e) = crate::utils::paste(text, app) {
            error!("Popup paste failed: {e}");
        }
    });
}
