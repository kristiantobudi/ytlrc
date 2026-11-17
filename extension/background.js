// ytlrc bridge — background service worker
// Picks the YouTube tab the user is actually watching and forwards its state
// to the local daemon.

const DAEMON_URL = "http://127.0.0.1:8765/state";
const REQUEST_TIMEOUT_MS = 2500;

// Must match `--token` (or YTLRC_TOKEN) on the daemon. Leave empty when the
// daemon only listens on 127.0.0.1, which is the default.
const TOKEN = "";

// Focused tabs always win. A playing tab keeps ownership only while no other
// tab has focus, so a paused or background tab can never overwrite the song
// that is actually playing.
let activeTabId = null;
let pendingState = null;
let inFlight = false;

chrome.runtime.onMessage.addListener((message, sender) => {
  if (!message || message.type !== "ytlrc:state" || !sender.tab) return;
  if (typeof sender.tab.id !== "number") return;

  const tabId = sender.tab.id;
  const state = message.state;

  if (sender.tab.active) {
    activeTabId = tabId;
  } else if (state.playing && activeTabId === null) {
    activeTabId = tabId;
  }

  // Ignore background tabs so they cannot overwrite the active song. Updates
  // from the active tab (including paused ones) always get through.
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
