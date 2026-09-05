// Prevents an extra console window on Windows in release builds.
// (The attribute is ignored by rustc on Linux and macOS.)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Component, Path, PathBuf};
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
    // Give headings `id`s so in-document `#fragment` links resolve.
    options.extension.header_ids = Some(String::new());
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

/// Resolve `.` and `..` lexically, without touching the filesystem.
///
/// `fs::canonicalize` is deliberately avoided: on Windows it returns a `\\?\`
/// verbatim path, and the OS refuses to resolve `..` against those, so the
/// *next* relative link from that document would fail to open.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Make `path` absolute against the working directory, then normalize it.
fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return normalize(path);
    }
    match std::env::current_dir() {
        Ok(cwd) => normalize(&cwd.join(path)),
        Err(_) => normalize(path),
    }
}

/// Turn a link `href` from the rendered document into a Markdown file path,
/// resolved relative to the document the link was clicked in.
fn resolve_markdown_href(current: Option<&Path>, href: &str) -> Result<PathBuf, String> {
    // Drop any #fragment or ?query the link carries; the frontend keeps the
    // fragment and scrolls to it once the new document has rendered.
    let raw = href.split(['#', '?']).next().unwrap_or("").trim();
    if raw.is_empty() {
        return Err("That link has no target.".to_string());
    }

    let candidate = PathBuf::from(raw);
    let joined = if candidate.is_absolute() {
        candidate
    } else {
        let dir = current.and_then(|p| p.parent()).ok_or_else(|| {
            "No document is open, so relative links cannot be resolved.".to_string()
        })?;
        dir.join(candidate)
    };

    let path = normalize(&joined);
    if !is_markdown(&path) {
        return Err(format!("Not a Markdown file: {}", path.display()));
    }
    if !path.is_file() {
        return Err(format!("File not found: {}", path.display()));
    }
    Ok(path)
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

/// Follow a Markdown link in the current document, replacing what the window
/// shows. Only `.md` / `.markdown` targets are accepted; everything else is
/// rejected here and handled by the frontend (external links go to the browser).
#[tauri::command]
fn open_markdown(
    window: tauri::Window,
    state: tauri::State<OpenedFile>,
    href: String,
) -> Result<FileResult, String> {
    let target = {
        let guard = state.0.lock().unwrap();
        resolve_markdown_href(guard.as_deref(), &href)?
    };

    let result = file_result(Some(&target));
    let _ = window.set_title(&window_title(Some(&target)));
    *state.0.lock().unwrap() = Some(target);
    Ok(result)
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
        .map(|p| absolutize(&p))
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
        .plugin(tauri_plugin_opener::init())
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
            open_markdown,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("md-preview-test-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn normalize_resolves_parent_segments() {
        let base = if cfg!(windows) { r"C:\a\b\c" } else { "/a/b/c" };
        let expected = if cfg!(windows) { r"C:\a\d" } else { "/a/d" };
        let input = Path::new(base).join("..").join("..").join("d");
        assert_eq!(normalize(&input), PathBuf::from(expected));
    }

    #[test]
    fn normalize_drops_current_dir_segments() {
        let base = if cfg!(windows) { r"C:\a" } else { "/a" };
        let expected = if cfg!(windows) { r"C:\a\b" } else { "/a/b" };
        let input = Path::new(base).join(".").join("b");
        assert_eq!(normalize(&input), PathBuf::from(expected));
    }

    #[test]
    fn normalize_stops_at_the_root() {
        let base = if cfg!(windows) { r"C:\" } else { "/" };
        let input = Path::new(base).join("..").join("..");
        assert_eq!(normalize(&input), PathBuf::from(base));
    }

    #[test]
    fn resolves_a_sibling_link() {
        let dir = temp_dir("sibling");
        let current = dir.join("README.md");
        std::fs::write(&current, "# hi").unwrap();
        std::fs::write(dir.join("ADR-001-thing.md"), "# adr").unwrap();

        let got = resolve_markdown_href(Some(&current), "ADR-001-thing.md").unwrap();
        assert_eq!(got, dir.join("ADR-001-thing.md"));
    }

    // The shape the example ADR index uses: `../feature-decisions/README.md`.
    #[test]
    fn resolves_a_parent_relative_link() {
        let root = temp_dir("parent-relative");
        let adr = root.join("adr");
        let siblings = root.join("feature-decisions");
        std::fs::create_dir_all(&adr).unwrap();
        std::fs::create_dir_all(&siblings).unwrap();

        let current = adr.join("README.md");
        std::fs::write(&current, "# index").unwrap();
        std::fs::write(siblings.join("README.md"), "# features").unwrap();

        let got = resolve_markdown_href(Some(&current), "../feature-decisions/README.md").unwrap();
        assert_eq!(got, siblings.join("README.md"));
    }

    // Following a link must leave the result usable as the base for the next
    // one -- the regression `fs::canonicalize` would cause on Windows.
    #[test]
    fn a_followed_link_can_itself_be_navigated_from() {
        let root = temp_dir("chained");
        let adr = root.join("adr");
        let specs = root.join("specs");
        std::fs::create_dir_all(&adr).unwrap();
        std::fs::create_dir_all(&specs).unwrap();

        let start = adr.join("README.md");
        std::fs::write(&start, "# index").unwrap();
        std::fs::write(adr.join("ADR-001.md"), "# one").unwrap();
        std::fs::write(specs.join("design.md"), "# design").unwrap();

        let hop1 = resolve_markdown_href(Some(&start), "ADR-001.md").unwrap();
        let hop2 = resolve_markdown_href(Some(&hop1), "../specs/design.md").unwrap();
        assert_eq!(hop2, specs.join("design.md"));
    }

    #[test]
    fn strips_fragment_and_query_before_resolving() {
        let dir = temp_dir("fragment");
        let current = dir.join("README.md");
        std::fs::write(&current, "# hi").unwrap();
        std::fs::write(dir.join("other.md"), "# other").unwrap();

        let got = resolve_markdown_href(Some(&current), "other.md#some-heading").unwrap();
        assert_eq!(got, dir.join("other.md"));
    }

    #[test]
    fn rejects_a_non_markdown_target() {
        let dir = temp_dir("non-md");
        let current = dir.join("README.md");
        std::fs::write(&current, "# hi").unwrap();
        std::fs::write(dir.join("diagram.png"), "not really a png").unwrap();

        let err = resolve_markdown_href(Some(&current), "diagram.png").unwrap_err();
        assert!(err.contains("Not a Markdown file"), "unexpected error: {err}");
    }

    #[test]
    fn reports_a_broken_link() {
        let dir = temp_dir("missing");
        let current = dir.join("README.md");
        std::fs::write(&current, "# hi").unwrap();

        let err = resolve_markdown_href(Some(&current), "ADR-999-nope.md").unwrap_err();
        assert!(err.contains("File not found"), "unexpected error: {err}");
    }

    #[test]
    fn refuses_relative_links_with_no_document_open() {
        let err = resolve_markdown_href(None, "other.md").unwrap_err();
        assert!(err.contains("No document is open"), "unexpected error: {err}");
    }

    #[test]
    fn accepts_an_absolute_target() {
        let dir = temp_dir("absolute");
        let target = dir.join("absolute.md");
        std::fs::write(&target, "# abs").unwrap();

        let got = resolve_markdown_href(None, &target.display().to_string()).unwrap();
        assert_eq!(got, target);
    }
}
