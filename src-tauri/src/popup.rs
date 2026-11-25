//! Writing-tools popup: shows a transcript and runs LLM prompts over it.

use crate::actions::{post_process_with_prompt, PostProcessOutcome, POST_PROCESS_TIMEOUT};
use crate::settings::get_settings;
use log::error;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewWindowBuilder};

const LABEL: &str = "popup";

// The window may still be loading when a transcript arrives, so the page
// reads the latest one on mount as well as from the event.
static LATEST_TEXT: Mutex<String> = Mutex::new(String::new());

pub fn show(app: &AppHandle, text: String) {
    *LATEST_TEXT.lock().unwrap() = text.clone();
    let window = match app.get_webview_window(LABEL) {
        Some(w) => w,
        None => {
            let mut builder = WebviewWindowBuilder::new(
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

/// Hides the popup so focus returns to the previous app, then pastes `text` there.
#[tauri::command]
#[specta::specta]
pub fn popup_paste(app: AppHandle, text: String) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.hide();
    }
    std::thread::spawn(move || {
        // shortcut: fixed wait for the WM to refocus the previous window; tune if pastes land in the wrong place.
        std::thread::sleep(Duration::from_millis(250));
        if let Err(e) = crate::utils::paste(text, app) {
            error!("Popup paste failed: {e}");
        }
    });
}
