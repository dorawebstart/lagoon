import { startClock } from "./components/pill.js";
import { trackHitRegion } from "./components/hit-region.js";

startClock(document.querySelector("#clock"));
trackHitRegion(document.querySelector("#island"));
