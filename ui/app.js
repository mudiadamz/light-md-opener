// Frontend entry. Uses the global Tauri API (app.withGlobalTauri = true),
// so no npm frontend dependencies are required.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// Schemes we hand to the OS default browser. Anything else is refused rather
// than letting the webview navigate away from the app, which it cannot undo.
const EXTERNAL_SCHEMES = ["http:", "https:", "mailto:", "tel:"];
const MARKDOWN_RE = /\.(md|markdown)$/i;
const SIDEBAR_KEY = "md-preview.sidebar";

const IS_MAC = /Mac|iPhone|iPad|iPod/.test(navigator.platform || navigator.userAgent);
const MOD_LABEL = IS_MAC ? "Cmd" : "Ctrl";
const ALT_LABEL = IS_MAC ? "Option" : "Alt";

// Absolute path of the document on screen, plus the trail either side of it.
let currentPath = null;
const backStack = [];
const forwardStack = [];

// { heading, link } pairs for the outline, rebuilt on every render.
let outlineEntries = [];

const el = (id) => document.getElementById(id);

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  }[c]));
}

// --- Transient status message -------------------------------------------

let toastTimer = null;
function toast(message) {
  let node = el("toast");
  if (!node) {
    node = document.createElement("div");
    node.id = "toast";
    document.body.appendChild(node);
  }
  node.textContent = message;
  node.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => node.classList.remove("show"), 3500);
}

// --- Outline --------------------------------------------------------------

// comrak puts the heading id on an inner `<a class="anchor">`, not the heading
// itself, so look there first and only fall back to a generated id.
function headingId(heading, index) {
  const anchor = heading.querySelector("a.anchor");
  const id = (anchor && anchor.id) || heading.id;
  if (id) return id;
  const generated = `heading-${index}`;
  heading.id = generated;
  return generated;
}

function buildOutline() {
  const nav = el("outline");
  nav.textContent = "";
  outlineEntries = [];

  const headings = Array.from(
    el("content").querySelectorAll("h1, h2, h3, h4, h5, h6")
  );

  if (headings.length === 0) {
    const empty = document.createElement("p");
    empty.className = "outline-empty";
    empty.textContent = "No headings in this document.";
    nav.appendChild(empty);
    return;
  }

  // Indent relative to the shallowest heading present, so a document that
  // starts at h2 is not pushed off to the right.
  const topLevel = Math.min(...headings.map((h) => Number(h.tagName[1])));

  headings.forEach((heading, i) => {
    const level = Number(heading.tagName[1]);
    const link = document.createElement("a");
    link.className = "outline-item";
    link.href = `#${headingId(heading, i)}`;
    link.dataset.level = String(level - topLevel + 1);
    link.style.paddingLeft = `${10 + (level - topLevel) * 14}px`;
    link.textContent = (heading.textContent || "").trim();
    link.title = link.textContent;
    nav.appendChild(link);
    outlineEntries.push({ heading, link });
  });

  updateActiveOutlineEntry();
}

function updateActiveOutlineEntry() {
  if (outlineEntries.length === 0) return;
  let active = outlineEntries[0];
  for (const entry of outlineEntries) {
    if (entry.heading.getBoundingClientRect().top <= 80) active = entry;
    else break;
  }
  for (const entry of outlineEntries) {
    entry.link.classList.toggle("active", entry === active);
  }
}

let scrollQueued = false;
window.addEventListener("scroll", () => {
  scheduleHideControls();
  if (scrollQueued) return;
  scrollQueued = true;
  requestAnimationFrame(() => {
    scrollQueued = false;
    updateActiveOutlineEntry();
  });
});

function setSidebar(open) {
  document.body.classList.toggle("with-sidebar", open);
  el("nav-sidebar").setAttribute("aria-expanded", String(open));
  try {
    localStorage.setItem(SIDEBAR_KEY, open ? "1" : "0");
  } catch (_) {
    // Private windows and blocked site data make storage throw; the toggle
    // still works for the rest of this session.
  }
}

function sidebarIsOpen() {
  return document.body.classList.contains("with-sidebar");
}

// --- Floating controls ----------------------------------------------------

// The controls start visible, hide five seconds after the reader scrolls, and
// come back on any click.
const CONTROLS_HIDE_MS = 5000;
let hideTimer = null;
let hidePending = false;

let pointerOverControls = false;

const controlGroups = () => [el("controls-nav"), el("controls-outline")];

function armHideControls() {
  clearTimeout(hideTimer);
  // Never let a pill disappear from under the pointer; the countdown resumes
  // when it leaves.
  if (pointerOverControls) return;
  hideTimer = setTimeout(() => {
    hidePending = false;
    document.body.classList.add("controls-hidden");
  }, CONTROLS_HIDE_MS);
}

function scheduleHideControls() {
  hidePending = true;
  armHideControls();
}

function showControls() {
  document.body.classList.remove("controls-hidden");
  clearTimeout(hideTimer);
  hidePending = false;
}

// --- Rendering ------------------------------------------------------------

function render(res, fragment) {
  const content = el("content");
  showControls();

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
    buildOutline();
    return;
  }

  // res.html is produced by comrak with raw-HTML escaping ON (safe).
  content.innerHTML = res.html;
  currentPath = res.path;
  // Window title is set natively from Rust; keep document.title in sync too.
  if (res.name) document.title = `${res.name} — MD Preview`;

  // Colorize fenced code blocks client-side.
  if (window.hljs) {
    content.querySelectorAll("pre code").forEach((node) => {
      try { window.hljs.highlightElement(node); } catch (_) { /* ignore */ }
    });
  }

  buildOutline();

  // Land at the top of the new document, or at the linked heading.
  window.scrollTo(0, 0);
  if (fragment) {
    const target = document.getElementById(fragment);
    if (target) target.scrollIntoView();
  }
  updateActiveOutlineEntry();
}

async function load() {
  try {
    // Rust reads the opened file (from argv) and returns rendered HTML.
    render(await invoke("get_opened_file"));
  } catch (e) {
    el("content").innerHTML =
      `<p class="notice error">Failed to load: ${escapeHtml(e)}</p>`;
  }
  updateNavButtons();
}

// --- Navigation -----------------------------------------------------------

function splitFragment(href) {
  const i = href.indexOf("#");
  return i === -1 ? [href, ""] : [href.slice(0, i), href.slice(i + 1)];
}

// Link targets are percent-encoded by some Markdown authors and not others;
// fall back to the raw text when the escape sequence is malformed.
function safeDecode(s) {
  try { return decodeURIComponent(s); } catch (_) { return s; }
}

function updateNavButtons() {
  el("nav-back").disabled = backStack.length === 0;
  el("nav-forward").disabled = forwardStack.length === 0;
}

// Load a Markdown document into the window. `record` runs only on success and
// is what moves paths between the back and forward stacks.
async function openDocument(href, record) {
  const [rawTarget, fragment] = splitFragment(href);
  const target = safeDecode(rawTarget);

  if (!MARKDOWN_RE.test(target)) {
    toast("Only Markdown files open in this window.");
    return false;
  }

  try {
    const res = await invoke("open_markdown", { href: target });
    const previous = currentPath;
    render(res, fragment);
    record(previous);
    updateNavButtons();
    return true;
  } catch (err) {
    toast(String(err));
    return false;
  }
}

// Following a new link discards anything that was ahead of us, the way a
// browser does.
function followLink(href) {
  return openDocument(href, (previous) => {
    if (previous) backStack.push(previous);
    forwardStack.length = 0;
  });
}

function goBack() {
  const target = backStack[backStack.length - 1];
  if (!target) return;
  openDocument(target, (previous) => {
    backStack.pop();
    if (previous) forwardStack.push(previous);
  });
}

function goForward() {
  const target = forwardStack[forwardStack.length - 1];
  if (!target) return;
  openDocument(target, (previous) => {
    forwardStack.pop();
    if (previous) backStack.push(previous);
  });
}

async function openExternal(url) {
  try {
    await invoke("plugin:opener|open_url", { url });
  } catch (err) {
    toast(`Could not open link: ${err}`);
  }
}

document.addEventListener("mousedown", showControls, true);

document.addEventListener("click", (e) => {
  // Leave modified clicks and non-primary buttons to the webview.
  if (e.defaultPrevented || e.button !== 0 || e.ctrlKey || e.metaKey || e.shiftKey) return;

  const anchor = e.target.closest && e.target.closest("a[href]");
  if (!anchor) return;

  const href = anchor.getAttribute("href");
  if (!href) return;

  // In-page anchor, including every outline entry: native scroll.
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
  followLink(href);
});

// --- Controls -------------------------------------------------------------

window.addEventListener("keydown", (e) => {
  const mod = IS_MAC ? e.metaKey : e.ctrlKey;

  if (mod && (e.key === "b" || e.key === "B")) {
    e.preventDefault();
    showControls();
    setSidebar(!sidebarIsOpen());
    return;
  }
  if (e.altKey && e.key === "ArrowLeft") {
    e.preventDefault();
    goBack();
    return;
  }
  if (e.altKey && e.key === "ArrowRight") {
    e.preventDefault();
    goForward();
    return;
  }
  if (e.key === "Backspace" && !e.ctrlKey && !e.metaKey) {
    e.preventDefault();
    if (e.shiftKey) goForward();
    else goBack();
  }
});

// Mouse thumb buttons: 3 is back, 4 is forward.
window.addEventListener("mouseup", (e) => {
  if (e.button === 3) {
    e.preventDefault();
    goBack();
  } else if (e.button === 4) {
    e.preventDefault();
    goForward();
  }
});

function wireControls() {
  const sidebarBtn = el("nav-sidebar");
  const backBtn = el("nav-back");
  const forwardBtn = el("nav-forward");

  sidebarBtn.title = `Toggle outline (${MOD_LABEL}+B)`;
  backBtn.title = `Back (${ALT_LABEL}+Left)`;
  forwardBtn.title = `Forward (${ALT_LABEL}+Right)`;

  for (const group of controlGroups()) {
    group.addEventListener("mouseenter", () => {
      pointerOverControls = true;
      clearTimeout(hideTimer);
    });
    group.addEventListener("mouseleave", () => {
      pointerOverControls = false;
      if (hidePending) armHideControls();
    });
  }

  sidebarBtn.addEventListener("click", () => setSidebar(!sidebarIsOpen()));
  backBtn.addEventListener("click", goBack);
  forwardBtn.addEventListener("click", goForward);

  let open = true;
  try {
    const stored = localStorage.getItem(SIDEBAR_KEY);
    if (stored !== null) open = stored === "1";
  } catch (_) {
    // Storage unavailable; fall back to showing the outline.
  }
  setSidebar(open);
}

// macOS hands "Open with" paths to the running app rather than through argv,
// so the backend pushes the rendered document when that happens.
listen("file-opened", (event) => {
  backStack.length = 0;
  forwardStack.length = 0;
  render(event.payload);
  updateNavButtons();
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

window.addEventListener("DOMContentLoaded", () => {
  wireControls();
  load();
});
