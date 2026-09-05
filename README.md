# MD Preview

Lightweight cross-platform Markdown preview app built with **Tauri v2** (Rust + the OS webview).
Open a `.md` / `.markdown` file from your file manager and see it rendered.

Runs on **Windows** (WebView2), **macOS** (WKWebView) and **Linux** (WebKitGTK).

- Renders **GitHub-Flavored Markdown** (tables, task lists, strikethrough, autolinks, footnotes) via `comrak`.
- Syntax-highlighted code blocks (highlight.js, vendored — works offline).
- Auto light/dark theme.
- Registers file associations so the app appears in the OS "Open with" list and can be made the default `.md` handler.
- Zoom with <kbd>Ctrl</kbd>/<kbd>Cmd</kbd> + <kbd>+</kbd>/<kbd>-</kbd>/<kbd>0</kbd> or <kbd>Ctrl</kbd>/<kbd>Cmd</kbd> + wheel.
- Tiny: a few MB per binary. No npm frontend build step (uses the global Tauri API).

## Requirements (to build)

Common to every platform:

- **Rust** (stable) — https://rustup.rs
- **Node.js** + npm (only to run the Tauri CLI)

Plus, per host OS:

| OS | Toolchain | Runtime |
| --- | --- | --- |
| Windows | Visual Studio Build Tools with the *Desktop development with C++* workload (`link.exe` + Windows SDK) | WebView2 — preinstalled on Windows 11 |
| macOS | Xcode Command Line Tools (`xcode-select --install`) | WKWebView — built in (macOS 10.15+) |
| Linux | `build-essential`, `pkg-config` | WebKitGTK — see packages below |

Linux build dependencies (Debian/Ubuntu; adjust names for other distros):

```bash
sudo apt install build-essential pkg-config curl wget file \
  libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev \
  libssl-dev libxdo-dev libayatana-appindicator3-dev
```

Tauri builds only for the host OS — there is no cross-compilation here. Produce each
platform's bundle on that platform (locally or in CI).

## Build & run

```bash
git clone https://github.com/mudiadamz/light-md-opener.git
cd light-md-opener

npm install                     # installs @tauri-apps/cli locally
npm run tauri dev               # run in dev
npm run tauri dev -- -- path/to/file.md   # dev, opening a file (args after `-- --`)
npm run tauri build             # produce the installers for the host OS
```

Bundle output, under `src-tauri/target/release/bundle/`:

| OS | Artifacts |
| --- | --- |
| Windows | `nsis/MD Preview_<version>_x64-setup.exe` (per-user, no admin; silent install with `/S`) |
| macOS | `macos/MD Preview.app`, `dmg/MD Preview_<version>_<arch>.dmg` |
| Linux | `deb/*.deb`, `rpm/*.rpm`, `appimage/*.AppImage` |

`bundle.targets` in `src-tauri/tauri.conf.json` lists every platform's targets; the bundler
silently ignores the ones that don't apply to the host.

## How the file association works

The association is declared once, in `bundle.fileAssociations` in `src-tauri/tauri.conf.json`,
and each bundler translates it for its platform.

### Windows

- The bundler registers a ProgID named **`Markdown Document`** with
  `shell\open\command = md-preview.exe "%1"` and sets it as the class default for `.md` / `.markdown`.
- Tauri's built-in macro does **not** add the ProgID to each extension's `OpenWithProgids` key
  (the canonical source for the "Open with" list). `src-tauri/nsis-hooks.nsh` fixes this in a
  `NSIS_HOOK_POSTINSTALL` hook (and cleans up on uninstall), so **MD Preview reliably appears in
  "Open with"** alongside your other editors.
- Windows 11 does not let an app silently become the default handler. Right-click a `.md` file →
  **Open with → Choose another app** → select **MD Preview** → check *Always use this app*.

### macOS

- The bundler writes `CFBundleDocumentTypes` into `Info.plist`, so Finder lists MD Preview under
  **Open With** once the `.app` has been seen by Launch Services (move it to `/Applications` and
  open it once).
- macOS does **not** pass the file path in `argv`. Finder delivers it as an Apple Event after
  launch, which arrives as `RunEvent::Opened`; `src-tauri/src/main.rs` handles that event, updates
  the stored path and pushes the rendered HTML to the frontend via a `file-opened` event.
- To make it the default: select a `.md` file in Finder, press <kbd>Cmd</kbd>+<kbd>I</kbd>, pick
  **MD Preview** under *Open with*, then click **Change All…**. There is no Default Apps settings
  pane on macOS.

### Linux

- The deb/rpm/AppImage bundles ship a `.desktop` entry with `MimeType=text/markdown`, which is
  what desktop environments read to build their "Open With" list.
- To make it the default: `xdg-mime default md-preview.desktop text/markdown`, or use your
  desktop's file-type settings (GNOME Settings → Default Applications, KDE System Settings →
  File Associations).

## Project structure

```
ui/                     static frontend (index.html, app.js, style.css, vendored highlight.js + CSS)
src-tauri/
  src/main.rs           argv / macOS-Opened capture -> comrak render -> get_opened_file
                        + open_default_apps_settings
  tauri.conf.json       product meta, fileAssociations, per-platform bundle config
  nsis-hooks.nsh        Windows only: adds OpenWithProgids so the app shows in "Open with"
  capabilities/         Tauri v2 permissions (core:default for the main window)
  icons/                generated from app-icon.png (includes .ico, .icns and PNGs)
samples/test.md         document exercising the renderer
```

## Deferred / possible enhancements

- Live reload (re-render on file change).
- Local relative images (enable the Tauri asset protocol for the file's directory).
- Mermaid diagrams, KaTeX math, in-app search, export to HTML/PDF.
- Single-instance + one window per opened file (matters most on macOS, where a running app is
  reused for subsequent "Open with" requests).
- A native menu bar on macOS (Cmd+W / Cmd+Q, zoom items).
- CI matrix (windows/macos/ubuntu runners) to produce all bundles per release.
- Code-sign and notarize the macOS `.app` and code-sign the Windows installer.
