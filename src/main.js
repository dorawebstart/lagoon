import { startClock, setClockLocale } from "./components/pill.js";
import { trackHitRegion } from "./components/hit-region.js";
import { setupExpand } from "./components/expand.js";
import { setupPlayer } from "./components/player.js";
import { setupVolume } from "./components/volume.js";
import { setupAgent } from "./components/agent.js";
import { setLanguage, getLocale } from "./i18n.js";
import { loadSettings, onSettingsChanged } from "./settings-store.js";

const island = document.querySelector("#island");

startClock(document.querySelectorAll("[data-clock]"), document.querySelector("#date"));
trackHitRegion(island);
const expander = setupExpand(island);
setupPlayer(island);
setupVolume(island);
const agent = setupAgent(island, expander);

/** Застосовує налаштування до острова: мову і колір. */
function applySettings(settings) {
  setLanguage(settings.language);
  setClockLocale(getLocale());
  island.style.setProperty("--island-color", settings.islandColor);
  agent.refresh();
}

loadSettings().then(applySettings);
// Змінили щось у вікні налаштувань — застосовуємо одразу, без перезапуску
onSettingsChanged(applySettings);
