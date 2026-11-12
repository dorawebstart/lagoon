// Медіа: отримуємо стан плеєра з Rust (media.rs).
// Поки що (крок 6) лише показуємо назву пісні рядком під датою;
// повноцінний плеєр з обкладинкою і кнопками з'явиться в кроці 7.
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

/**
 * Викликає onUpdate(state | null) зараз і після кожної зміни.
 * null — нічого не грає.
 */
export async function watchMedia(onUpdate) {
  await listen("media:update", (event) => onUpdate(event.payload));
  // Стан міг з'явитися ще до того, як сторінка підписалася — запитуємо його
  onUpdate(await invoke("media_current"));
}

/** Тимчасовий показ: «▶ Пісня — Виконавець». */
export function renderNowPlaying(el, state) {
  if (!state || !state.title) {
    el.hidden = true;
    return;
  }
  el.hidden = false;
  const icon = state.isPlaying ? "▶" : "❚❚";
  el.textContent = `${icon} ${state.title}${state.artist ? " — " + state.artist : ""}`;
}
