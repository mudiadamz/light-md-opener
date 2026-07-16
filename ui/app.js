// Frontend entry. Uses the global Tauri API (app.withGlobalTauri = true),
// so no npm frontend dependencies are required.
const { invoke } = window.__TAURI__.core;

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  }[c]));
}

async function load() {
  const content = document.getElementById("content");
  try {
    // Rust reads the opened file (from argv) and returns rendered HTML.
    const res = await invoke("get_opened_file");

    if (res.error) {
      content.innerHTML = `<p class="notice error">${escapeHtml(res.error)}</p>`;
      return;
    }

    if (!res.path) {
      content.innerHTML =
        `<div class="notice">` +
        `<h1>MD Preview</h1>` +
        `<p>Open a <code>.md</code> file from Explorer to preview it here.</p>` +
        `</div>`;
      return;
    }

    // res.html is produced by comrak with raw-HTML escaping ON (safe).
    content.innerHTML = res.html;
    // Window title is set natively from Rust; keep document.title in sync too.
    if (res.name) document.title = `${res.name} — MD Preview`;

    // Colorize fenced code blocks client-side.
    if (window.hljs) {
      content.querySelectorAll("pre code").forEach((el) => {
        try { window.hljs.highlightElement(el); } catch (_) { /* ignore */ }
      });
    }
  } catch (e) {
    content.innerHTML = `<p class="notice error">Failed to load: ${escapeHtml(e)}</p>`;
  }
}

// --- Zoom: Ctrl +/-/0 and Ctrl + mouse wheel ---
let zoom = 1;
const ZOOM_MIN = 0.3;
const ZOOM_MAX = 5;
const ZOOM_STEP = 0.1;

function applyZoom() {
  // CSS `zoom` reflows content (unlike transform:scale) and is supported by WebView2.
  document.body.style.zoom = String(zoom);
}
function setZoom(z) {
  zoom = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, Math.round(z * 100) / 100));
  applyZoom();
}

window.addEventListener("keydown", (e) => {
  if (!e.ctrlKey) return;
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
    if (!e.ctrlKey) return;
    e.preventDefault(); // stop page scroll while zooming
    setZoom(zoom + (e.deltaY < 0 ? ZOOM_STEP : -ZOOM_STEP));
  },
  { passive: false }
);

window.addEventListener("DOMContentLoaded", load);
