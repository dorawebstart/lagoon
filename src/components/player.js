// Плеєр на острові: стан приходить з Rust (media.rs), кнопки керують плеєром
// через команду media_control.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/**
 * @typedef {object} MediaState
 * @property {string} title
 * @property {string} artist
 * @property {string} album
 * @property {string} appId
 * @property {boolean} isPlaying
 * @property {number} positionMs
 * @property {number} durationMs
 * @property {number} updatedAtMs
 * @property {string | null} thumbnail
 */

/** 75000 → "1:15", 3725000 → "1:02:05" */
function formatTime(ms) {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}

export function setupPlayer(island) {
  const covers = island.querySelectorAll("[data-cover]");
  const title = island.querySelector("#track-title");
  const artist = island.querySelector("#track-artist");
  const elapsed = island.querySelector("#time-elapsed");
  const left = island.querySelector("#time-left");
  const fill = island.querySelector("#bar-fill");

  /** @type {MediaState | null} */
  let state = null;
  let frame = 0;

  /** Поточна позиція: остання відома + скільки минуло, якщо грає. */
  function currentPosition() {
    if (!state) return 0;
    const drift = state.isPlaying ? Date.now() - state.updatedAtMs : 0;
    return Math.min(state.durationMs, state.positionMs + Math.max(0, drift));
  }

  function renderProgress() {
    if (!state || state.durationMs <= 0) return;
    const position = currentPosition();
    elapsed.textContent = formatTime(position);
    left.textContent = "-" + formatTime(state.durationMs - position);
    fill.style.transform = `scaleX(${position / state.durationMs})`;
  }

  // Прогрес оновлюємо лише коли його видно (острів розгорнутий і грає музика)
  function tick() {
    renderProgress();
    frame = requestAnimationFrame(tick);
  }
  function syncTicker() {
    const visible = island.classList.contains("is-expanded") && state?.isPlaying;
    if (visible && !frame) frame = requestAnimationFrame(tick);
    if (!visible && frame) {
      cancelAnimationFrame(frame);
      frame = 0;
    }
  }
  // Острів розгорнули/згорнули — вмикаємо/вимикаємо оновлення прогресу
  new MutationObserver(syncTicker).observe(island, { attributes: true, attributeFilter: ["class"] });

  /** @param {MediaState | null} next */
  function render(next) {
    state = next && next.title ? next : null;
    const hasMedia = state !== null;

    island.classList.toggle("has-media", hasMedia);
    island.classList.toggle("is-playing", hasMedia && state.isPlaying);
    island.classList.toggle("no-timeline", !hasMedia || state.durationMs <= 0);

    if (hasMedia) {
      title.textContent = state.title;
      artist.textContent = state.artist;
      for (const cover of covers) {
        cover.style.backgroundImage = state.thumbnail ? `url("${state.thumbnail}")` : "";
        cover.classList.toggle("has-image", Boolean(state.thumbnail));
      }
      renderProgress();
    }
    syncTicker();
  }

  // Кнопки ⏮ ⏯ ⏭
  island.querySelectorAll("[data-action]").forEach((button) => {
    button.addEventListener("click", async () => {
      const action = button.dataset.action;
      // Оптимістично: одразу міняємо іконку, не чекаючи підтвердження від Windows
      if (action === "playPause" && state) {
        render({ ...state, positionMs: currentPosition(), updatedAtMs: Date.now(), isPlaying: !state.isPlaying });
      }
      try {
        await invoke("media_control", { action });
      } catch (error) {
        console.warn("media_control:", error);
      }
    });
  });

  listen("media:update", (event) => render(event.payload));
  // Стан міг з'явитися ще до того, як сторінка підписалася — запитуємо його
  invoke("media_current").then(render);
}
