# MD Preview (win32)

Lightweight Windows Markdown preview app built with **Tauri v2** (Rust + WebView2).
Open a `.md` / `.markdown` file from Explorer and see it rendered.

- Renders **GitHub-Flavored Markdown** (tables, task lists, strikethrough, autolinks, footnotes) via `comrak`.
- Syntax-highlighted code blocks (highlight.js, vendored — works offline).
- Auto light/dark theme.
- Registers file associations so it appears in Explorer's **"Open with"** list and can be set as the default `.md` handler.
- Tiny: ~4 MB app exe, ~1.4 MB installer. No npm frontend build step (uses the global Tauri API).

## Requirements (to build)

- **Rust** (stable, MSVC toolchain) — https://rustup.rs
- **Visual Studio Build Tools** with the *Desktop development with C++* workload (provides `link.exe` + Windows SDK)
- **Node.js** + npm (only to run the Tauri CLI)
- **WebView2** runtime — preinstalled on Windows 11

## Build & run

```powershell
npm install                         # installs @tauri-apps/cli locally
npm run tauri dev                   # run in dev
npm run tauri dev -- -- "path\to\file.md"   # dev, opening a file (args after `-- --`)
npm run tauri build                 # produce the NSIS installer
```

Installer output:
`src-tauri/target/release/bundle/nsis/MD Preview_<version>_x64-setup.exe`

Install it (per-user, no admin). Silent install: `"MD Preview_..._x64-setup.exe" /S`.

## How the file association works

- The Tauri bundler (`bundle.fileAssociations` in `src-tauri/tauri.conf.json`) registers a ProgID
  named **`Markdown Document`** with `shell\open\command = md-preview.exe "%1"` and sets it as the
  class default for `.md` / `.markdown`.
- Tauri's built-in macro does **not** add the ProgID to each extension's `OpenWithProgids` key
  (the canonical source for the "Open with" list). `src-tauri/nsis-hooks.nsh` fixes this in a
  `NSIS_HOOK_POSTINSTALL` hook (and cleans up on uninstall), so **MD Preview reliably appears in
  "Open with"** alongside your other editors.

### Setting it as the default (Windows 11)

Windows 11 does **not** allow an app to silently become the default handler — you confirm it once:

- Right-click a `.md` file → **Open with → Choose another app** → select **MD Preview** → check
  *Always use this app*, **or**
- Click **"Set as default…"** in the app toolbar (opens Settings → Default apps).

## Project structure

```
ui/                     static frontend (index.html, app.js, style.css, vendored highlight.js + CSS)
src-tauri/
  src/main.rs           argv capture -> comrak render -> get_opened_file / open_default_apps_settings
  tauri.conf.json       product meta, fileAssociations, NSIS config
  nsis-hooks.nsh        adds OpenWithProgids so the app shows in "Open with"
  capabilities/         Tauri v2 permissions (core:default for the main window)
  icons/                generated from app-icon.png
samples/test.md         document exercising the renderer
```

## Deferred / possible enhancements

- Live reload (re-render on file change).
- Local relative images (enable the Tauri asset protocol for the file's directory).
- Mermaid diagrams, KaTeX math, in-app search, export to HTML/PDF.
- Single-instance + one window per opened file.
- Code-sign the installer to avoid SmartScreen warnings.
