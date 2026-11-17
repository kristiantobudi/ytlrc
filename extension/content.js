// ytlrc bridge — content script
// Membaca lagu + posisi playback dari halaman YouTube lalu mengirimkannya
// ke background service worker (yang meneruskan ke daemon localhost).
(() => {
  if (window.top !== window) return; // cukup frame utama

  const isMusic = location.hostname === "music.youtube.com";
  const HEARTBEAT_MS = 5000;

  let last = { at: 0, position: -1, paused: null, videoId: null, ad: null, title: null };

  const textOf = (selector) => {
    const element = document.querySelector(selector);
    return element ? (element.textContent || "").trim() : "";
  };

  const getVideo = () =>
    document.querySelector("video.html5-main-video") ||
    document.querySelector("ytmusic-player video") ||
    document.querySelector("video");

  const getVideoId = () => {
    try {
      return new URL(location.href).searchParams.get("v") || "";
    } catch {
      return "";
    }
  };

  const isAd = () => {
    if (document.querySelector(".ad-showing")) return true;
    if (document.querySelector(".ytp-ad-player-overlay")) return true;
    if (document.querySelector(".ytp-ad-module .ytp-ad-player-overlay-layout")) return true;
    return false;
  };

  const getTitle = () => {
    if (isMusic) {
      return (
        textOf("ytmusic-player-bar .title") ||
        textOf("ytmusic-player-bar yt-formatted-string.title") ||
        (navigator.mediaSession?.metadata?.title || "").trim()
      );
    }

    return (
      textOf("ytd-watch-metadata #title h1") ||
      textOf("#title h1 yt-formatted-string") ||
      textOf("h1.ytd-watch-metadata") ||
      textOf("#title h1") ||
      (navigator.mediaSession?.metadata?.title || "").trim() ||
      document.title.replace(/\s*-\s*YouTube\s*$/, "")
    );
  };

  const getArtist = () => {
    if (isMusic) {
      const fromByline = textOf("ytmusic-player-bar .byline a");
      const byline = textOf("ytmusic-player-bar .byline");
      return (
        fromByline ||
        byline.split("•")[0].trim() ||
        (navigator.mediaSession?.metadata?.artist || "").trim()
      );
    }
    return (
      textOf("#owner #channel-name a") ||
      textOf("ytd-channel-name a") ||
      document.querySelector('meta[itemprop="author"]')?.content ||
      (navigator.mediaSession?.metadata?.artist || "").trim()
    );
  };

  const readState = () => {
    const video = getVideo();
    if (!video) return null;

    const title = getTitle();
    if (!title) return null;

    return {
      videoId: getVideoId(),
      title,
      artist: getArtist(),
      channel: getArtist(),
      duration: Number.isFinite(video.duration) ? video.duration : 0,
      position: Number.isFinite(video.currentTime) ? video.currentTime : 0,
      paused: Boolean(video.paused),
      isAd: isAd(),
      source: isMusic ? "youtube-music" : "youtube",
      playing: !video.paused && !isAd(),
    };
  };

  const tick = () => {
    const state = readState();
    if (!state) return;

    const now = Date.now();
    const changed =
      state.videoId !== last.videoId ||
      state.paused !== last.paused ||
      state.isAd !== last.ad ||
      state.title !== last.title ||
      Math.abs(state.position - last.position) > 0.35 ||
      now - last.at > HEARTBEAT_MS;

    if (!changed) return;

    last = {
      at: now,
      position: state.position,
      paused: state.paused,
      videoId: state.videoId,
      ad: state.isAd,
      title: state.title,
    };

    try {
      const result = chrome.runtime.sendMessage({ type: "ytlrc:state", state });
      if (result && typeof result.catch === "function") {
        result.catch(() => {});
      }
    } catch {
      // Service worker sedang restart; tick berikutnya akan mencoba lagi.
    }
  };

  setInterval(tick, 1000);
  tick();
})();
