// ytlrc bridge — background service worker
// Memilih tab YouTube yang benar-benar berbunyi, lalu POST state ke daemon.

const DAEMON_URL = "http://127.0.0.1:8765/state";

let activeTabId = null;
let pending = null;
let inFlight = false;

chrome.runtime.onMessage.addListener((message, sender) => {
  if (!message || message.type !== "ytlrc:state" || !sender.tab) return;
  if (typeof sender.tab.id !== "number") return;

  const tabId = sender.tab.id;
  const state = message.state;

  if (state.playing) activeTabId = tabId;
  else if (activeTabId === null) activeTabId = tabId;

  // Abaikan tab lain yang hanya paused/iklan supaya tidak menimpa lagu aktif.
  if (tabId !== activeTabId && !state.playing) return;

  pending = state;
  if (!inFlight) void flush();
});

chrome.tabs.onRemoved.addListener((tabId) => {
  if (tabId === activeTabId) activeTabId = null;
});

async function flush() {
  if (!pending) return;
  inFlight = true;

  const state = pending;
  pending = null;

  try {
    await fetch(DAEMON_URL, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(state),
    });
  } catch {
    // Daemon belum jalan / port beda. Diamkan; update berikutnya mencoba lagi.
  } finally {
    inFlight = false;
    if (pending) void flush();
  }
}
