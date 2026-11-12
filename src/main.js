import { startClock, setClockLocale } from "./components/pill.js";
import { trackHitRegion } from "./components/hit-region.js";
import { setupExpand } from "./components/expand.js";
import { setupPlayer } from "./components/player.js";
import { setupVolume } from "./components/volume.js";
import { setupAgent } from "./components/agent.js";
import { setupLyrics } from "./components/lyrics.js";
import { setupDrag } from "./components/drag.js";
import { setupAccent } from "./components/accent.js";
import { setLanguage, getLocale } from "./i18n.js";
import { loadSettings, onSettingsChanged } from "./settings-store.js";

const island = document.querySelector("#island");

startClock(document.querySelectorAll("[data-clock]"), document.querySelector("#date"));
trackHitRegion(island);
const expander = setupExpand(island);
setupPlayer(island);
setupVolume(island);
const agent = setupAgent(island, expander);
setupDrag(island, expander);
const accent = setupAccent();
const lyrics = setupLyrics({
  stage: document.querySelector("#stage"),
  track: document.querySelector("#lyrics-track"),
  toggle: document.querySelector("#lyrics-toggle"),
});

/** Застосовує налаштування до острова: мову, тему і показ тексту пісні. */
function applySettings(settings) {
  setLanguage(settings.language);
  setClockLocale(getLocale());
  applyTheme(settings.theme, settings.islandColor);
  accent.setMode(settings.accentMode, settings.accentColor);
  applyTextColor(settings.textColor);
  applySizes(settings);
  lyrics.setEnabled(settings.showLyrics);
  agent.refresh();
}

/**
 * Тема — атрибут data-theme на <body> (кольори задає island.css).
 * «Свій колір» — значення змінної --island-bg на <html> (:root): його
 * успадковують обидва острови, основний і з текстом пісні.
 */
function applyTheme(theme, color) {
  document.body.dataset.theme = theme;
  if (theme === "custom") {
    document.documentElement.style.setProperty("--island-bg", color);
  } else {
    document.documentElement.style.removeProperty("--island-bg");
  }
}

/** Свій колір тексту — у --island-fg на :root; порожній рядок — стандартний (білий). */
function applyTextColor(color) {
  const root = document.documentElement.style;
  if (color) root.setProperty("--island-fg", color);
  else root.removeProperty("--island-fg");
}

/**
 * Розміри: масштаб острова, ширина й шрифт тексту пісні — CSS-змінні на :root.
 * Розмір самого вікна під них підганяє Rust (position.rs, Layout).
 */
function applySizes(settings) {
  const root = document.documentElement.style;
  root.setProperty("--scale", String(settings.islandScale / 100));
  root.setProperty("--lyrics-width", `${settings.lyricsWidth}px`);
  root.setProperty("--lyrics-font", `${settings.lyricsFont}px`);
  // Висота рядка ≈ 1.65 × шрифт: 12 px → 20 px, як було до кроку 18
  root.setProperty("--lyrics-line", `${Math.round(settings.lyricsFont * 1.65)}px`);
  lyrics.relayout();
}

loadSettings().then(applySettings);
// Змінили щось у вікні налаштувань — застосовуємо одразу, без перезапуску
onSettingsChanged(applySettings);
