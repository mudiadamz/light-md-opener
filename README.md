# MD Preview

Lightweight cross-platform Markdown preview app built with **Tauri v2** (Rust + the OS webview).
Open a `.md` / `.markdown` file from your file manager and see it rendered.

Runs on **Windows** (WebView2), **macOS** (WKWebView) and **Linux** (WebKitGTK).

- Renders **GitHub-Flavored Markdown** (tables, task lists, strikethrough, autolinks, footnotes) via `comrak`.
- Syntax-highlighted code blocks (highlight.js, vendored — works offline).
- Auto light/dark theme.
- **Follows Markdown links in place** - clicking a `.md` link loads that document in the
  same window, with browser-style back/forward history. Web links open in your browser instead.
- **Outline sidebar** listing the document's headings, with the current section highlighted.
  Toggle it from the floating button or <kbd>Ctrl</kbd>/<kbd>Cmd</kbd>+<kbd>B</kbd>; the choice is remembered.
- **Controls stay out of the way** - two floating pills that fade out five seconds after you
  scroll and return on a click, so nothing sits permanently over the document.
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

## Following links

Links in the rendered document are classified by target:

| Link | Behaviour |
| --- | --- |
| `ADR-001.md`, `../specs/design.md` | Loads in the same window, resolved relative to the current document |
| `#some-heading` | Scrolls within the document (comrak emits heading anchors) |
| `https://`, `http://`, `mailto:`, `tel:` | Handed to the OS default browser or mail client |
| Any other scheme, or a non-Markdown file | Refused, with a short message |

Navigation history works like a browser's: following a new link discards anything that was
ahead of you.

Every absolute URL is intercepted before the webview sees it. Letting the webview navigate to a
remote page would replace the app's own UI with no way back, so that is prevented rather than
merely discouraged.

Broken links, non-Markdown targets and refused schemes report why instead of failing silently.

## Keyboard and mouse

| Action | Shortcut |
| --- | --- |
| Back / Forward | <kbd>Alt</kbd>+<kbd>&larr;</kbd> / <kbd>Alt</kbd>+<kbd>&rarr;</kbd> (<kbd>Option</kbd> on macOS) |
| Back / Forward | <kbd>Backspace</kbd> / <kbd>Shift</kbd>+<kbd>Backspace</kbd> |
| Back / Forward | Mouse thumb buttons |
| Toggle the outline | <kbd>Ctrl</kbd>/<kbd>Cmd</kbd>+<kbd>B</kbd> |
| Zoom in / out / reset | <kbd>Ctrl</kbd>/<kbd>Cmd</kbd> + <kbd>+</kbd> / <kbd>-</kbd> / <kbd>0</kbd> |
| Zoom | <kbd>Ctrl</kbd>/<kbd>Cmd</kbd> + mouse wheel, or trackpad pinch |

The window has no fixed chrome. Two rounded, translucent pills float over the document instead:

- **Top left** - back and forward. They grey out when there is nowhere to go, and sit clear of the
  outline when it is open.
- **Top right** - the outline toggle.

Both are visible when a document opens, fade out five seconds after you scroll, and come back on a
click anywhere. Resting the pointer on a pill keeps it from disappearing mid-reach; the countdown
resumes when the pointer leaves. The file name lives in the window title bar rather than in the
chrome.

The outline nests by heading level and highlights the section you are reading as you scroll.
Clicking an entry jumps to that heading.

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

## Releases

`.github/workflows/release.yml` builds every platform's bundles on a runner of that OS
(Tauri cannot cross-compile) and attaches them to a GitHub Release.

```bash
# bump the version in package.json and src-tauri/tauri.conf.json first, then:
git tag v0.1.0
git push origin v0.1.0
```

The workflow creates a draft release, uploads the Windows, macOS (Apple Silicon + Intel)
and Linux bundles into it, and publishes it once all four matrix jobs succeed. It can also
be started by hand from the Actions tab (`workflow_dispatch`).

The binaries are **not code-signed**: Windows SmartScreen and macOS Gatekeeper will warn on
first launch. See the enhancements list below.

## Project structure

```
ui/                     static frontend: index.html (floating controls + outline sidebar + document),
                        app.js (link following, history, outline, auto-hiding
                        controls, zoom), style.css,
                        vendored highlight.js + CSS
src-tauri/
  src/main.rs           argv / macOS-Opened capture -> comrak render; commands:
                        get_opened_file, open_markdown (in-window link following),
                        open_default_apps_settings
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
- Code-sign and notarize the macOS `.app` and code-sign the Windows installer.
