// ytlrc bridge — content script
// Reads the track that is currently playing plus its playback position and
// forwards it to the background service worker, which posts it to the local
// daemon.
(() => {
  if (window.top !== window) return; // main frame only

  const isMusic = location.hostname === "music.youtube.com";
  const TICK_MS = 1000;

  // Last payload sent, used to avoid flooding the daemon with identical
  // updates. Playback position alone keeps it live while a song is playing.
  let last = {
    at: 0,
    position: -1,
    paused: null,
    videoId: "",
    ad: null,
    title: "",
    artist: "",
  };

  const textOf = (selector) => {
    const element = document.querySelector(selector);
    return element ? (element.textContent || "").trim() : "";
  };

  const getVideo = () =>
    document.querySelector("video.html5-main-video") ||
    document.querySelector("ytmusic-player video") ||
    document.querySelector("video");

  const videoIdFrom = (url) => {
    try {
      const parsed = new URL(url, location.href);
      const fromQuery = parsed.searchParams.get("v");
      if (fromQuery) return fromQuery;
      const match = parsed.pathname.match(/^\/(?:shorts|embed|live)\/([\w-]{6,})/);
      return match ? match[1] : "";
    } catch {
      return "";
    }
  };

  const getVideoId = () => {
    const fromPage = videoIdFrom(location.href);
    if (fromPage) return fromPage;
    const video = getVideo();
    return video ? videoIdFrom(video.currentSrc || video.src) : "";
  };

  const mediaMetadata = () => {
    try {
      return navigator.mediaSession ? navigator.mediaSession.metadata : null;
    } catch {
      return null;
    }
  };

  const stripPageSuffix = (value) =>
    value.replace(/\s*[-–—]\s*YouTube(?:\s*Music)?\s*$/i, "").trim();

  // YouTube updates mediaSession on every track change, so it is the most
  // reliable source. DOM lookups are only a fallback: after an SPA navigation
  // the metadata nodes of the previous video can still be in the document.
  const getTitle = () => {
    const metadata = mediaMetadata();
    if (metadata && metadata.title && metadata.title.trim()) {
      return metadata.title.trim();
    }

    const pageTitle = stripPageSuffix(document.title || "");
    if (pageTitle) return pageTitle;

    if (isMusic) {
      return (
        textOf("ytmusic-player-bar .title") ||
        textOf("ytmusic-player-bar yt-formatted-string.title")
      );
    }
    return (
      textOf("ytd-watch-metadata #title h1") ||
      textOf("#title h1 yt-formatted-string") ||
      textOf("h1.ytd-watch-metadata") ||
      textOf("#title h1")
    );
  };

  const getArtist = () => {
    const metadata = mediaMetadata();
    if (metadata && metadata.artist && metadata.artist.trim()) {
      return metadata.artist.trim();
    }
    if (isMusic) {
      const fromByline = textOf("ytmusic-player-bar .byline a");
      const byline = textOf("ytmusic-player-bar .byline");
      return fromByline || byline.split("•")[0].trim();
    }
    return (
      textOf("#owner #channel-name a") ||
      textOf("ytd-channel-name a") ||
      document.querySelector('meta[itemprop="author"]')?.content ||
      ""
    );
  };

  const isVisible = (element) =>
    !!element &&
    (element.offsetWidth > 0 ||
      element.offsetHeight > 0 ||
      element.getClientRects().length > 0);

  const isAd = () => {
    const player =
      document.querySelector("#movie_player") || document.querySelector("ytd-player");
    if (player && player.classList.contains("ad-showing")) return true;
    // Only treat the overlay as an ad while it is really visible; YouTube
    // keeps the node in the DOM (hidden) after an ad has finished.
    return isVisible(document.querySelector(".ytp-ad-player-overlay"));
  };

  const readState = () => {
    const video = getVideo();
    if (!video) return null;

    const title = getTitle();
    if (!title) return null;

    const artist = getArtist();
    return {
      videoId: getVideoId(),
      title,
      artist,
      channel: artist,
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
      state.title !== last.title ||
      state.artist !== last.artist ||
      state.paused !== last.paused ||
      state.isAd !== last.ad ||
      Math.abs(state.position - last.position) > 0.35 ||
      now - last.at > 3000;

    if (!changed) return;

    last = {
      at: now,
      position: state.position,
      paused: state.paused,
      videoId: state.videoId,
      ad: state.isAd,
      title: state.title,
      artist: state.artist,
    };

    try {
      const result = chrome.runtime.sendMessage({ type: "ytlrc:state", state });
      if (result && typeof result.catch === "function") result.catch(() => {});
    } catch {
      // Service worker is restarting; the next tick retries.
    }
  };

  // YouTube is a SPA: read immediately after a navigation instead of waiting
  // for the next heartbeat.
  for (const event of ["yt-navigate-finish", "yt-page-data-updated", "yt-player-updated"]) {
    document.addEventListener(event, () => {
      last.at = 0;
      tick();
    });
  }

  setInterval(tick, TICK_MS);
  tick();
})();
