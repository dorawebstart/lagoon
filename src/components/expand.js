// Розкриття острова при наведенні.
// Про наведення повідомляє Rust подією "island:hover" (див. window.rs),
// бо коли вікно «прозоре для миші», сторінка сама не бачить курсора.
//
// Крім наведення, острів можна «закріпити» розкритим (pin) — наприклад,
// поки Claude Code чекає дозволу. Тоді він не згортається, навіть якщо курсор пішов.
//
// Поки острів перетягують мишею (drag.js), розкриття призупинено (suspend):
// острів згорнутий, а після перетягування (resume) знову реагує як завжди.
import { listen } from "@tauri-apps/api/event";

/** Через скільки мс після відходу курсора острів згортається. */
const COLLAPSE_DELAY = 400;
/** Затримка перед розкриттям: курсор, що просто пролітає повз, острів не розкриває. */
const HOVER_DELAY = 150;

/**
 * @returns {{ pin: () => void, unpin: () => void, suspend: () => void, resume: () => void }}
 */
export function setupExpand(island) {
  let collapseTimer = null;
  let expandTimer = null;
  let hovered = false;
  let pinned = false;
  let suspended = false;

  const expand = () => {
    clearTimeout(collapseTimer);
    if (!suspended) island.classList.add("is-expanded");
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
    clearTimeout(expandTimer);
    if (hovered) {
      clearTimeout(collapseTimer);
      // Уже розкритий (курсор повернувся до згортання) — лишаємо як є
      if (island.classList.contains("is-expanded")) return;
      expandTimer = setTimeout(expand, HOVER_DELAY);
    } else {
      collapseLater();
    }
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
    suspend() {
      suspended = true;
      clearTimeout(collapseTimer);
      island.classList.remove("is-expanded");
    },
    resume() {
      suspended = false;
      if (pinned || hovered) expand();
    },
  };
}
