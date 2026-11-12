import { startClock, setClockLocale } from "./components/pill.js";
import { trackHitRegion } from "./components/hit-region.js";
import { setupExpand } from "./components/expand.js";
import { setupPlayer } from "./components/player.js";
import { setupVolume } from "./components/volume.js";
import { setupAgent } from "./components/agent.js";
import { setupLyrics } from "./components/lyrics.js";
import { setLanguage, getLocale } from "./i18n.js";
import { loadSettings, onSettingsChanged } from "./settings-store.js";

const island = document.querySelector("#island");
const lyricsBox = document.querySelector("#lyrics");

startClock(document.querySelectorAll("[data-clock]"), document.querySelector("#date"));
trackHitRegion(island);
const expander = setupExpand(island);
setupPlayer(island);
setupVolume(island);
const agent = setupAgent(island, expander);
const lyrics = setupLyrics({
  stage: document.querySelector("#stage"),
  track: document.querySelector("#lyrics-track"),
  toggle: document.querySelector("#lyrics-toggle"),
});

/** Застосовує налаштування до острова: мову, колір і показ тексту пісні. */
function applySettings(settings) {
  setLanguage(settings.language);
  setClockLocale(getLocale());
  // Колір — обом островам: основному і острівцю з текстом
  for (const element of [island, lyricsBox]) {
    element.style.setProperty("--island-color", settings.islandColor);
  }
  lyrics.setEnabled(settings.showLyrics);
  agent.refresh();
}

loadSettings().then(applySettings);
// Змінили щось у вікні налаштувань — застосовуємо одразу, без перезапуску
onSettingsChanged(applySettings);
