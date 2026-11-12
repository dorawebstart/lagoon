// Розкриття острова при наведенні.
// Про наведення повідомляє Rust подією "island:hover" (див. window.rs),
// бо коли вікно «прозоре для миші», сторінка сама не бачить курсора.
//
// Крім наведення, острів можна «закріпити» розкритим (pin) — наприклад,
// поки Claude Code чекає дозволу. Тоді він не згортається, навіть якщо курсор пішов.
import { listen } from "@tauri-apps/api/event";

/** Через скільки мс після відходу курсора острів згортається. */
const COLLAPSE_DELAY = 400;

/**
 * @returns {{ pin: () => void, unpin: () => void }}
 */
export function setupExpand(island) {
  let collapseTimer = null;
  let hovered = false;
  let pinned = false;

  const expand = () => {
    clearTimeout(collapseTimer);
    island.classList.add("is-expanded");
  };

  const collapseLater = () => {
    clearTimeout(collapseTimer);
    collapseTimer = setTimeout(() => {
      if (!pinned && !hovered) island.classList.remove("is-expanded");
    }, COLLAPSE_DELAY);
  };

  // event.payload — те, що Rust передав у emit (true / false)
  listen("island:hover", (event) => {
    hovered = Boolean(event.payload);
    if (hovered) expand();
    else collapseLater();
  });

  return {
    pin() {
      pinned = true;
      expand();
    },
    unpin() {
      pinned = false;
      if (!hovered) collapseLater();
    },
  };
}
