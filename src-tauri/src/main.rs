// Prevents an extra console window on Windows in release builds.
// (The attribute is ignored by rustc on Linux and macOS.)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use comrak::{markdown_to_html, ComrakOptions};
use serde::Serialize;
use tauri::Manager;

/// The Markdown file this instance was launched to open, if any.
///
/// Windows and Linux pass the path in argv. macOS does not: Finder/`open`
/// deliver it *after* launch via `RunEvent::Opened`, which updates this.
struct OpenedFile(Mutex<Option<PathBuf>>);

#[derive(Clone, Serialize)]
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

fn is_markdown(path: &Path) -> bool {
    match path.extension() {
        Some(ext) => {
            let ext = ext.to_string_lossy().to_ascii_lowercase();
            ext == "md" || ext == "markdown"
        }
        None => false,
    }
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

/// Read and render `path`, or produce the empty "no file" result.
fn file_result(path: Option<&Path>) -> FileResult {
    let Some(path) = path else {
        return FileResult {
            path: None,
            name: None,
            html: String::new(),
            error: None,
        };
    };

    let name = path.file_name().map(|s| s.to_string_lossy().into_owned());
    match std::fs::read_to_string(path) {
        Ok(content) => FileResult {
            path: Some(path.display().to_string()),
            name,
            html: render_markdown(&content),
            error: None,
        },
        Err(e) => FileResult {
            path: Some(path.display().to_string()),
            name,
            html: String::new(),
            error: Some(format!("Could not read file: {e}")),
        },
    }
}

fn window_title(path: Option<&Path>) -> String {
    match path.and_then(|p| p.file_name()) {
        Some(name) => format!("{} — MD Preview", name.to_string_lossy()),
        None => "MD Preview".to_string(),
    }
}

/// Called by the frontend on startup to fetch the rendered document.
#[tauri::command]
fn get_opened_file(state: tauri::State<OpenedFile>) -> FileResult {
    let guard = state.0.lock().unwrap();
    file_result(guard.as_deref())
}

/// Try each candidate `[program, args...]` in order; succeed on the first that starts.
#[cfg(any(target_os = "windows", target_os = "linux"))]
fn spawn_first(candidates: &[&[&str]]) -> Result<(), String> {
    let mut last = "no supported settings application found".to_string();
    for candidate in candidates {
        let Some((program, args)) = candidate.split_first() else {
            continue;
        };
        match std::process::Command::new(program).args(args).spawn() {
            Ok(_) => return Ok(()),
            Err(e) => last = format!("{program}: {e}"),
        }
    }
    Err(last)
}

/// Open the OS UI for choosing default applications. No desktop platform lets
/// an app claim a default handler silently, so this only takes the user there.
#[tauri::command]
fn open_default_apps_settings() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    return spawn_first(&[&["cmd", "/C", "start", "", "ms-settings:defaultapps"]]);

    #[cfg(target_os = "macos")]
    return Err("macOS has no Default Apps pane. Select the .md file in Finder, \
                press Cmd-I, pick MD Preview under \"Open with\", then \"Change All…\"."
        .to_string());

    #[cfg(target_os = "linux")]
    return spawn_first(&[
        &["gnome-control-center", "default-apps"],
        &["systemsettings", "kcm_filetypes"],
        &["systemsettings5", "kcm_filetypes"],
        &["xfce4-mime-settings"],
    ]);

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    return Err("Not supported on this platform.".to_string());
}

/// Pick the first argv entry that looks like a Markdown file path.
/// Also skips macOS's `-psn_*` Finder argument, which has no `.md` extension.
fn opened_file_from_args() -> Option<PathBuf> {
    std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .find(|p| is_markdown(p))
}

/// Swap in a newly opened document at runtime (macOS "Open with" / `open`).
#[cfg(target_os = "macos")]
fn set_opened_file(app: &tauri::AppHandle, path: PathBuf) {
    use tauri::Emitter;

    if let Some(state) = app.try_state::<OpenedFile>() {
        *state.0.lock().unwrap() = Some(path.clone());
    }
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.set_title(&window_title(Some(&path)));
        let _ = win.set_focus();
    }
    let _ = app.emit("file-opened", file_result(Some(&path)));
}

fn main() {
    let opened = opened_file_from_args();
    // Window title shows the opened file name (there is no in-app toolbar).
    let title = window_title(opened.as_deref());

    tauri::Builder::default()
        .manage(OpenedFile(Mutex::new(opened)))
        .setup(move |app| {
            if let Some(win) = app.get_webview_window("main") {
                // Maximize on launch (belt-and-suspenders alongside
                // `maximized: true` in tauri.conf.json).
                let _ = win.maximize();
                // Put the file name in the native title bar.
                let _ = win.set_title(&title);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_opened_file,
            open_default_apps_settings
        ])
        .build(tauri::generate_context!())
        .expect("error while building MD Preview")
        .run(|_app, _event| {
            // macOS delivers "Open with" paths through Apple Events, not argv.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = _event {
                if let Some(path) = urls
                    .iter()
                    .filter_map(|u| u.to_file_path().ok())
                    .find(|p| is_markdown(p))
                {
                    set_opened_file(_app, path);
                }
            }
        });
}
