// Текст пісні: окремий острівець ліворуч від основного.
// Три рядки — щойно проспіваний, поточний (яскравий) і наступний (сіріші).
// Текст шукає Rust (lyrics.rs, LRCLIB); тут лише показуємо його в такт пісні.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { positionOf } from "./player.js";
import { saveSettings } from "../settings-store.js";

/** Висота одного рядка в px — має збігатися з .lyrics__line у island.css. */
const LINE_HEIGHT = 20;

/**
 * @typedef {object} LyricLine
 * @property {number} timeMs
 * @property {string} text
 */

/**
 * @param {object} elements
 * @param {HTMLElement} elements.stage   спільний контейнер обох островів
 * @param {HTMLElement} elements.track   «стрічка» з усіма рядками, що їде вгору
 * @param {HTMLButtonElement} elements.toggle  кнопка 🎤 у плеєрі
 */
export function setupLyrics({ stage, track, toggle }) {
  /** @type {LyricLine[]} */
  let lines = [];
  /** Яку пісню зараз показуємо (щоб не шукати текст повторно на кожну паузу). */
  let songKey = null;
  let media = null;
  let enabled = true;
  let currentIndex = -2;
  let timer = 0;

  /** Ключ пісні: виконавець + назва + тривалість. */
  function keyOf(state) {
    return state ? `${state.artist}\u001f${state.title}\u001f${Math.round(state.durationMs / 1000)}` : null;
  }

  /** Шукає текст нової пісні. */
  async function loadLyrics(state) {
    const key = keyOf(state);
    songKey = key;
    setLines([]);
    // Тривалість ще невідома (браузер щойно відкрив відео) — чекаємо: без неї
    // LRCLIB не знайде пісню, а ключ зміниться, щойно тривалість з'явиться.
    if (!state || state.durationMs <= 0) return;

    try {
      const result = await invoke("lyrics_get", {
        artist: state.artist,
        title: state.title,
        album: state.album,
        durationMs: state.durationMs,
      });
      // Поки чекали відповідь, пісня могла змінитися — тоді відповідь застаріла
      if (songKey !== key) return;
      setLines(result?.synced ?? []);
    } catch (error) {
      console.warn("lyrics_get:", error);
    }
  }

  /** Перебудовує «стрічку» рядків. Порожній текст = програш, показуємо ♪. */
  function setLines(next) {
    // Вступ до першого рядка: додаємо порожній рядок на 0:00, щоб посередині
    // острівця був ♪, а не порожнеча з одним сірим рядком унизу.
    lines = next.length && next[0].timeMs > 0 ? [{ timeMs: 0, text: "" }, ...next] : next;
    track.replaceChildren(
      ...lines.map((line) => {
        const row = document.createElement("div");
        row.className = "lyrics__line";
        row.textContent = line.text || "♪";
        return row;
      }),
    );
    currentIndex = -2;
    sync();
    updateVisibility();
  }

  /** Індекс останнього рядка, що вже почався (-1 — пісня ще до першого рядка). */
  function indexAt(positionMs) {
    // Бінарний пошук: рядки відсортовані за часом
    let low = 0;
    let high = lines.length - 1;
    let found = -1;
    while (low <= high) {
      const middle = (low + high) >> 1;
      if (lines[middle].timeMs <= positionMs) {
        found = middle;
        low = middle + 1;
      } else {
        high = middle - 1;
      }
    }
    return found;
  }

  /** Ставить рядок `index` посередині острівця і робить його яскравим. */
  function showLine(index) {
    if (index === currentIndex) return;
    track.children[currentIndex]?.classList.remove("is-current");
    track.children[index]?.classList.add("is-current");
    currentIndex = index;
    // Рядок index має опинитися в другому (середньому) ряду з трьох
    track.style.transform = `translateY(${(1 - index) * LINE_HEIGHT}px)`;
  }

  /**
   * Показує поточний рядок і ставить таймер рівно на початок наступного —
   * так не треба перевіряти позицію 60 разів на секунду.
   */
  function sync() {
    clearTimeout(timer);
    if (!lines.length || !media) return;
    const position = positionOf(media);
    const index = indexAt(position);
    showLine(index);
    const next = lines[index + 1];
    if (media.isPlaying && next) {
      timer = setTimeout(sync, Math.max(30, next.timeMs - position));
    }
  }

  function updateVisibility() {
    const available = lines.length > 0 && media !== null;
    // has-lyrics — текст є (видно кнопку 🎤); show-lyrics — острівець показано
    stage.classList.toggle("has-lyrics", available);
    stage.classList.toggle("show-lyrics", available && enabled);
    toggle.setAttribute("aria-pressed", String(enabled));
  }

  function onMedia(state) {
    media = state && state.title ? state : null;
    if (keyOf(media) !== songKey) {
      loadLyrics(media);
    } else {
      // Та сама пісня: пауза, перемотка — просто пересинхронізуємося
      sync();
      updateVisibility();
    }
  }

  toggle.addEventListener("click", () => {
    enabled = !enabled;
    updateVisibility();
    saveSettings({ showLyrics: enabled }).catch((error) => console.warn("showLyrics:", error));
  });

  listen("media:update", (event) => onMedia(event.payload));
  invoke("media_current").then(onMedia);

  return {
    /** Застосовує налаштування showLyrics (при старті і зі зміни в іншому вікні). */
    setEnabled(value) {
      enabled = value !== false;
      updateVisibility();
    },
  };
}
