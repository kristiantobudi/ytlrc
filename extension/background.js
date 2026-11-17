// ytlrc bridge — background service worker
// Picks the YouTube tab the user is actually watching and forwards its state
// to the local daemon.

const DAEMON_URL = "http://127.0.0.1:8765/state";
const HEALTH_URL = "http://127.0.0.1:8765/health";
const REQUEST_TIMEOUT_MS = 2500;
const HEALTH_CHECK_INTERVAL_MS = 60_000;

// Must match `--token` (or YTLRC_TOKEN) on the daemon. Leave empty when the
// daemon only listens on 127.0.0.1, which is the default.
const TOKEN = "";

let versionMismatchWarned = false;

async function checkDaemonVersion() {
  try {
    const response = await fetch(HEALTH_URL, { method: "GET" });
    if (!response.ok) return;
    const body = await response.json();
    const daemonVersion = body.version;
    const extensionVersion = chrome.runtime.getManifest().version;
    if (daemonVersion && daemonVersion !== extensionVersion && !versionMismatchWarned) {
      versionMismatchWarned = true;
      console.warn(
        `ytlrc: extension v${extensionVersion} talking to daemon v${daemonVersion}. ` +
          "Update one of them to match if lyrics look wrong."
      );
    }
  } catch {
    // Daemon not running yet; the next interval retries.
  }
}

checkDaemonVersion();
setInterval(checkDaemonVersion, HEALTH_CHECK_INTERVAL_MS);

// Only the tab the user is actually watching should drive the daemon: the
// active tab of the currently focused browser window. This also resolves
// multi-window setups where several tabs can report `tab.active === true`
// at once (one per window) — without tracking window focus, the last one to
// post would silently win.
let focusedWindowId = chrome.windows.WINDOW_ID_NONE;
let activeTabId = null;
let pendingState = null;
let inFlight = false;

chrome.windows.onFocusChanged.addListener((windowId) => {
  // WINDOW_ID_NONE fires when focus leaves the browser entirely (another
  // app, the OS). Keep the previous window as the owner instead of
  // dropping ownership, so playback does not pause lyrics just because the
  // user alt-tabbed away.
  if (windowId !== chrome.windows.WINDOW_ID_NONE) {
    focusedWindowId = windowId;
  }
});

chrome.windows.getLastFocused({}, (window) => {
  if (window && !chrome.runtime.lastError) focusedWindowId = window.id;
});

function isOwnerCandidate(tab, state) {
  if (!tab.active) return false;
  // Window focus is unknown at service-worker startup; don't block on it.
  if (focusedWindowId !== chrome.windows.WINDOW_ID_NONE && tab.windowId !== focusedWindowId) {
    // A tab that is both audible and playing can still take over — this
    // covers picture-in-picture-style setups where the user watches a
    // background window on purpose.
    return Boolean(tab.audible) && Boolean(state.playing);
  }
  return true;
}

chrome.runtime.onMessage.addListener((message, sender) => {
  if (!message || message.type !== "ytlrc:state" || !sender.tab) return;
  if (typeof sender.tab.id !== "number") return;

  const tabId = sender.tab.id;
  const state = message.state;

  if (isOwnerCandidate(sender.tab, state)) {
    activeTabId = tabId;
  } else if (state.playing && activeTabId === null) {
    activeTabId = tabId;
  }

  // Ignore tabs that are not the current owner so they cannot overwrite the
  // song that is actually playing.
  if (tabId !== activeTabId) return;

  pendingState = state;
  if (!inFlight) void flush();
});

chrome.tabs.onRemoved.addListener((tabId) => {
  if (tabId === activeTabId) activeTabId = null;
});

async function flush() {
  if (inFlight || !pendingState) return;
  inFlight = true;

  const state = pendingState;
  pendingState = null;

  // A daemon that stops responding must never leave `inFlight` stuck forever,
  // otherwise every later update would be dropped until the page is reloaded.
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);

  try {
    const headers = { "Content-Type": "application/json" };
    if (TOKEN) headers.Authorization = `Bearer ${TOKEN}`;

    await fetch(DAEMON_URL, {
      method: "POST",
      headers,
      body: JSON.stringify(state),
      signal: controller.signal,
    });
  } catch {
    // Daemon not running / different port. The next update retries.
  } finally {
    clearTimeout(timeout);
    inFlight = false;
    if (pendingState) void flush();
  }
}
