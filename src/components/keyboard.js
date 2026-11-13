// Розкладка клавіатури: коли змінили мову вводу, пігулка на 1,5 с показує
// «UA» / «EN» поруч з годинником (подію "keyboard:layout" шле Rust, keyboard.rs).
//
// Ширина пігулки не змінюється: на час спалаху мітка стає на місце
// погоди, батареї чи еквалайзера (клас has-layout, див. island.css).
import { listen } from "@tauri-apps/api/event";

/** Скільки мс показувати мітку. */
const FLASH_MS = 1500;

export function setupKeyboard(island) {
  const badge = island.querySelector("#layout-badge");
  let enabled = true;
  let timer = null;

  listen("keyboard:layout", (event) => {
    if (!enabled) return;
    badge.textContent = String(event.payload);
    island.classList.add("has-layout");
    clearTimeout(timer);
    timer = setTimeout(() => island.classList.remove("has-layout"), FLASH_MS);
  });

  return {
    /** Перемикач «Показувати зміну розкладки» з налаштувань. */
    setEnabled(value) {
      enabled = value !== false;
      if (!enabled) island.classList.remove("has-layout");
    },
  };
}
