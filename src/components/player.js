// Плеєр на острові: стан приходить з Rust (media.rs), кнопки керують плеєром
// через команду media_control, а прогрес-бар перемотує через media_seek
// (лише якщо плеєр це дозволяє — поле canSeek).
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
 * @property {boolean} canSeek
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

/**
 * Поточна позиція пісні (мс): остання відома + скільки минуло, якщо грає.
 * Експортуємо, бо те саме потрібно тексту пісні (lyrics.js).
 * @param {MediaState} state
 */
export function positionOf(state) {
  const drift = state.isPlaying ? Date.now() - state.updatedAtMs : 0;
  return Math.min(state.durationMs, state.positionMs + Math.max(0, drift));
}

export function setupPlayer(island) {
  const covers = island.querySelectorAll("[data-cover]");
  const title = island.querySelector("#track-title");
  const artist = island.querySelector("#track-artist");
  const elapsed = island.querySelector("#time-elapsed");
  const left = island.querySelector("#time-left");
  const fill = island.querySelector("#bar-fill");
  const seek = island.querySelector("#seek");

  /** @type {MediaState | null} */
  let state = null;
  let frame = 0;
  /** Позиція (мс) під курсором, поки тягнемо прогрес-бар; null — не тягнемо. */
  let dragPosition = null;

  /** Поточна позиція: остання відома + скільки минуло, якщо грає. */
  function currentPosition() {
    return state ? positionOf(state) : 0;
  }

  function renderProgress() {
    if (!state || state.durationMs <= 0) return;
    // Під час перетягування показуємо позицію під курсором, а не реальну
    const position = dragPosition ?? currentPosition();
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
    island.classList.toggle("can-seek", canSeek());

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

  /** Чи можна зараз перемотувати: є пісня, відома тривалість і плеєр дозволяє. */
  function canSeek() {
    return state !== null && state.canSeek && state.durationMs > 0;
  }

  /** Перетворює координату курсора на позицію в пісні (мс). */
  function positionAt(event) {
    const rect = seek.getBoundingClientRect();
    const fraction = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
    return fraction * state.durationMs;
  }

  function stopDragging() {
    dragPosition = null;
    island.classList.remove("is-seeking");
  }

  // Перемотка: натиснули → тягнемо (показуємо позицію) → відпустили → media_seek.
  // setPointerCapture — події миші йдуть сюди, навіть якщо курсор вийшов за смужку.
  seek.addEventListener("pointerdown", (event) => {
    if (!canSeek() || event.button !== 0) return;
    event.preventDefault();
    seek.setPointerCapture(event.pointerId);
    dragPosition = positionAt(event);
    island.classList.add("is-seeking");
    renderProgress();
  });

  seek.addEventListener("pointermove", (event) => {
    if (dragPosition === null) return;
    dragPosition = positionAt(event);
    renderProgress();
  });

  seek.addEventListener("pointerup", async () => {
    if (dragPosition === null) return;
    const target = Math.round(dragPosition);
    stopDragging();
    // Оптимістично: одразу показуємо нову позицію, не чекаючи підтвердження
    render({ ...state, positionMs: target, updatedAtMs: Date.now() });
    try {
      await invoke("media_seek", { positionMs: target });
    } catch (error) {
      console.warn("media_seek:", error);
    }
  });

  // Захоплення втрачено без pointerup (наприклад, курсор пішов з острова
  // і вікно стало «прозорим для миші») — скасовуємо перемотку.
  seek.addEventListener("lostpointercapture", () => {
    if (dragPosition === null) return;
    stopDragging();
    renderProgress();
  });

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
