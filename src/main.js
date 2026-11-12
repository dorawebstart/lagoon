import { startClock } from "./components/pill.js";
import { trackHitRegion } from "./components/hit-region.js";
import { setupExpand } from "./components/expand.js";

const island = document.querySelector("#island");

startClock(document.querySelectorAll("[data-clock]"), document.querySelector("#date"));
trackHitRegion(island);
setupExpand(island);
