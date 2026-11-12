import { startClock, setClockLocale } from "./components/pill.js";
import { trackHitRegion } from "./components/hit-region.js";
import { setupExpand } from "./components/expand.js";
import { setupPlayer } from "./components/player.js";
import { setLanguage, getLocale } from "./i18n.js";
import { loadSettings, onSettingsChanged } from "./settings-store.js";

const island = document.querySelector("#island");

/** Застосовує налаштування до острова: мову і колір. */
function applySettings(settings) {
  setLanguage(settings.language);
  setClockLocale(getLocale());
  island.style.setProperty("--island-color", settings.islandColor);
}

startClock(document.querySelectorAll("[data-clock]"), document.querySelector("#date"));
trackHitRegion(island);
setupExpand(island);
setupPlayer(island);

loadSettings().then(applySettings);
// Змінили щось у вікні налаштувань — застосовуємо одразу, без перезапуску
onSettingsChanged(applySettings);
