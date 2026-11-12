// Розкриття острова при наведенні.
// Про наведення повідомляє Rust подією "island:hover" (див. window.rs),
// бо коли вікно «прозоре для миші», сторінка сама не бачить курсора.
import { listen } from "@tauri-apps/api/event";

/** Через скільки мс після відходу курсора острів згортається. */
const COLLAPSE_DELAY = 400;

export async function setupExpand(island) {
  let collapseTimer = null;

  const expand = () => {
    clearTimeout(collapseTimer);
    island.classList.add("is-expanded");
  };

  const collapseLater = () => {
    clearTimeout(collapseTimer);
    collapseTimer = setTimeout(() => island.classList.remove("is-expanded"), COLLAPSE_DELAY);
  };

  // event.payload — те, що Rust передав у emit (true / false)
  await listen("island:hover", (event) => {
    if (event.payload) expand();
    else collapseLater();
  });
}
