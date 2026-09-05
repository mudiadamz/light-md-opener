// Frontend entry. Uses the global Tauri API (app.withGlobalTauri = true),
// so no npm frontend dependencies are required.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// Schemes we hand to the OS default browser. Anything else is refused rather
// than letting the webview navigate away from the app, which it cannot undo.
const EXTERNAL_SCHEMES = ["http:", "https:", "mailto:", "tel:"];
const MARKDOWN_RE = /\.(md|markdown)$/i;

// Absolute path of the document on screen, and the trail that led to it.
let currentPath = null;
const backStack = [];

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  }[c]));
}

// --- Transient status message -------------------------------------------

let toastTimer = null;
function toast(message) {
  let el = document.getElementById("toast");
  if (!el) {
    el = document.createElement("div");
    el.id = "toast";
    document.body.appendChild(el);
  }
  el.textContent = message;
  el.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove("show"), 3500);
}

// --- Rendering ------------------------------------------------------------

function render(res, fragment) {
  const content = document.getElementById("content");

  if (res.error) {
    content.innerHTML = `<p class="notice error">${escapeHtml(res.error)}</p>`;
    return;
  }

  if (!res.path) {
    content.innerHTML =
      `<div class="notice">` +
      `<h1>MD Preview</h1>` +
      `<p>Open a <code>.md</code> file from your file manager to preview it here.</p>` +
      `</div>`;
    return;
  }

  // res.html is produced by comrak with raw-HTML escaping ON (safe).
  content.innerHTML = res.html;
  currentPath = res.path;
  // Window title is set natively from Rust; keep document.title in sync too.
  if (res.name) document.title = `${res.name} — MD Preview`;

  // Colorize fenced code blocks client-side.
  if (window.hljs) {
    content.querySelectorAll("pre code").forEach((el) => {
      try { window.hljs.highlightElement(el); } catch (_) { /* ignore */ }
    });
  }

  // Land at the top of the new document, or at the linked heading.
  window.scrollTo(0, 0);
  if (fragment) {
    const target = document.getElementById(fragment);
    if (target) target.scrollIntoView();
  }
}

async function load() {
  try {
    // Rust reads the opened file (from argv) and returns rendered HTML.
    render(await invoke("get_opened_file"));
  } catch (e) {
    document.getElementById("content").innerHTML =
      `<p class="notice error">Failed to load: ${escapeHtml(e)}</p>`;
  }
}

// --- Link handling --------------------------------------------------------

function splitFragment(href) {
  const i = href.indexOf("#");
  return i === -1 ? [href, ""] : [href.slice(0, i), href.slice(i + 1)];
}

// Link targets are percent-encoded by some Markdown authors and not others;
// fall back to the raw text when the escape sequence is malformed.
function safeDecode(s) {
  try { return decodeURIComponent(s); } catch (_) { return s; }
}

// Load another Markdown document into this window. `push` is false when we
// are stepping backwards, so the back stack is not re-grown.
async function navigate(href, { push = true } = {}) {
  const [rawTarget, fragment] = splitFragment(href);
  const target = safeDecode(rawTarget);

  if (!MARKDOWN_RE.test(target)) {
    toast("Only Markdown files open in this window.");
    return;
  }

  const from = currentPath;
  try {
    const res = await invoke("open_markdown", { href: target });
    if (push && from) backStack.push(from);
    render(res, fragment);
  } catch (err) {
    toast(String(err));
  }
}

async function openExternal(url) {
  try {
    await invoke("plugin:opener|open_url", { url });
  } catch (err) {
    toast(`Could not open link: ${err}`);
  }
}

function goBack() {
  const previous = backStack.pop();
  if (previous) navigate(previous, { push: false });
}

document.addEventListener("click", (e) => {
  // Leave modified clicks and non-primary buttons to the webview.
  if (e.defaultPrevented || e.button !== 0 || e.ctrlKey || e.metaKey || e.shiftKey) return;

  const anchor = e.target.closest && e.target.closest("a[href]");
  if (!anchor) return;

  const href = anchor.getAttribute("href");
  if (!href) return;

  // In-page anchor: let the webview do its native scroll.
  if (href.startsWith("#")) return;

  const scheme = /^([a-z][a-z0-9+.-]*):/i.exec(href);
  if (scheme) {
    // Every absolute URL is intercepted; the webview must never navigate.
    e.preventDefault();
    const protocol = `${scheme[1].toLowerCase()}:`;
    if (EXTERNAL_SCHEMES.includes(protocol)) openExternal(href);
    else toast(`Refusing to open a ${protocol} link.`);
    return;
  }

  e.preventDefault();
  navigate(href);
});

// Alt+Left and Backspace walk back through followed links; so does the
// mouse's dedicated back button.
window.addEventListener("keydown", (e) => {
  if ((e.altKey && e.key === "ArrowLeft") || (e.key === "Backspace" && !e.ctrlKey && !e.metaKey)) {
    e.preventDefault();
    goBack();
  }
});
window.addEventListener("mouseup", (e) => {
  if (e.button === 3) {
    e.preventDefault();
    goBack();
  }
});

// macOS hands "Open with" paths to the running app rather than through argv,
// so the backend pushes the rendered document when that happens.
listen("file-opened", (event) => {
  backStack.length = 0;
  render(event.payload);
});

// --- Zoom: Ctrl/Cmd +/-/0 and Ctrl/Cmd + mouse wheel ---
let zoom = 1;
const ZOOM_MIN = 0.3;
const ZOOM_MAX = 5;
const ZOOM_STEP = 0.1;

function applyZoom() {
  // CSS `zoom` reflows content (unlike transform:scale) and is supported by
  // WebView2, WKWebView and WebKitGTK.
  document.body.style.zoom = String(zoom);
}
function setZoom(z) {
  zoom = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, Math.round(z * 100) / 100));
  applyZoom();
}

window.addEventListener("keydown", (e) => {
  // Cmd is the zoom modifier on macOS, Ctrl everywhere else.
  if (!e.ctrlKey && !e.metaKey) return;
  // "=" is the unshifted Ctrl++ on most layouts; also accept "+" and numpad.
  if (e.key === "=" || e.key === "+" || e.code === "NumpadAdd") {
    e.preventDefault();
    setZoom(zoom + ZOOM_STEP);
  } else if (e.key === "-" || e.code === "NumpadSubtract") {
    e.preventDefault();
    setZoom(zoom - ZOOM_STEP);
  } else if (e.key === "0" || e.code === "Numpad0") {
    e.preventDefault();
    setZoom(1); // reset
  }
});

window.addEventListener(
  "wheel",
  (e) => {
    // Trackpad pinch arrives as a ctrlKey wheel event on every platform.
    if (!e.ctrlKey && !e.metaKey) return;
    e.preventDefault(); // stop page scroll while zooming
    setZoom(zoom + (e.deltaY < 0 ? ZOOM_STEP : -ZOOM_STEP));
  },
  { passive: false }
);

window.addEventListener("DOMContentLoaded", load);
