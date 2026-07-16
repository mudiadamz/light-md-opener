// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Mutex;

use comrak::{markdown_to_html, ComrakOptions};
use serde::Serialize;
use tauri::Manager;

/// The Markdown file this instance was launched to open (from argv), if any.
struct OpenedFile(Mutex<Option<PathBuf>>);

#[derive(Serialize)]
struct FileResult {
    /// Absolute path of the opened file, or null when launched with no file.
    path: Option<String>,
    /// File name for the title bar / toolbar.
    name: Option<String>,
    /// Rendered HTML (raw-HTML in the source is escaped — safe to inject).
    html: String,
    /// Populated when the file exists but could not be read.
    error: Option<String>,
}

/// Render Markdown to HTML with GitHub-flavored extensions.
/// `render.unsafe_` stays false, so embedded raw HTML is escaped (XSS-safe).
fn render_markdown(md: &str) -> String {
    let mut options = ComrakOptions::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.autolink = true;
    options.extension.footnotes = true;
    options.extension.tagfilter = true;
    // Emit `<pre><code class="language-xxx">` so highlight.js can colorize.
    options.render.github_pre_lang = true;
    markdown_to_html(md, &options)
}

/// Called by the frontend on startup to fetch the rendered document.
#[tauri::command]
fn get_opened_file(state: tauri::State<OpenedFile>) -> FileResult {
    let guard = state.0.lock().unwrap();
    match guard.as_ref() {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(content) => FileResult {
                path: Some(path.display().to_string()),
                name: path
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned()),
                html: render_markdown(&content),
                error: None,
            },
            Err(e) => FileResult {
                path: Some(path.display().to_string()),
                name: path.file_name().map(|s| s.to_string_lossy().into_owned()),
                html: String::new(),
                error: Some(format!("Could not read file: {e}")),
            },
        },
        None => FileResult {
            path: None,
            name: None,
            html: String::new(),
            error: None,
        },
    }
}

/// Open Windows Settings > Default apps so the user can make MD Preview the
/// default `.md` handler. Windows 11 forbids setting this silently.
#[tauri::command]
fn open_default_apps_settings() -> Result<(), String> {
    std::process::Command::new("cmd")
        .args(["/C", "start", "", "ms-settings:defaultapps"])
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Pick the first argv entry that looks like a Markdown file path.
fn opened_file_from_args() -> Option<PathBuf> {
    std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .find(|p| match p.extension() {
            Some(ext) => {
                let ext = ext.to_string_lossy().to_ascii_lowercase();
                ext == "md" || ext == "markdown"
            }
            None => false,
        })
}

fn main() {
    let opened = opened_file_from_args();

    // Window title shows the opened file name (there is no in-app toolbar).
    let window_title = match opened.as_ref().and_then(|p| p.file_name()) {
        Some(name) => format!("{} — MD Preview", name.to_string_lossy()),
        None => "MD Preview".to_string(),
    };

    tauri::Builder::default()
        .manage(OpenedFile(Mutex::new(opened)))
        .setup(move |app| {
            if let Some(win) = app.get_webview_window("main") {
                // Maximize on launch (belt-and-suspenders alongside
                // `maximized: true` in tauri.conf.json).
                let _ = win.maximize();
                // Put the file name in the native title bar.
                let _ = win.set_title(&window_title);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_opened_file,
            open_default_apps_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running MD Preview");
}
