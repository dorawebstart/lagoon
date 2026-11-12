// Перетягування острова мишею і його положення (вгорі / внизу).
//
// Відрізняємо перетягування від кліку: поки курсор зсунувся менше ніж на
// 5 px від точки натискання — це звичайний клік. Більше — перетягування:
// повідомляємо Rust (island_drag_start), а далі вікно за курсором рухає вже
// він (position.rs) і сам помічає, коли кнопку відпустили (подія island:drag-end).
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Поріг у px: менший рух — клік, більший — перетягування. */
const DRAG_THRESHOLD = 5;

/** За ці елементи не тягнемо: кнопки, повзунки, прогрес-бар. */
const NO_DRAG = "button, input, a, .player__seek";

/**
 * @param {HTMLElement} island
 * @param {{ suspend: () => void, resume: () => void }} expander
 */
export function setupDrag(island, expander) {
  /** Де натиснули: { x, y, id } або null. */
  let start = null;
  let dragging = false;
  /** Погасити наступний click (щоб перетягування не натиснуло кнопку). */
  let swallowClick = false;

  island.addEventListener("pointerdown", (event) => {
    if (event.button !== 0 || dragging) return;
    swallowClick = false;
    if (event.target.closest(NO_DRAG)) return;
    // screenX/screenY — координати на екрані: вони не залежать від того, що вікно рухається
    start = { x: event.screenX, y: event.screenY, id: event.pointerId };
  });

  island.addEventListener("pointermove", (event) => {
    if (!start || dragging || event.pointerId !== start.id) return;
    const distance = Math.hypot(event.screenX - start.x, event.screenY - start.y);
    if (distance < DRAG_THRESHOLD) return;

    // Поріг перейдено — це перетягування
    dragging = true;
    swallowClick = true;
    start = null;
    document.body.classList.add("is-dragging");
    // Згортаємо острів і не розкриваємо від наведення, поки тягнемо
    expander.suspend();
    invoke("island_drag_start").catch((error) => {
      console.warn("island_drag_start:", error);
      endDrag();
    });
  });

  const cancelPress = () => {
    start = null;
  };
  island.addEventListener("pointerup", cancelPress);
  island.addEventListener("pointercancel", cancelPress);

  // Фаза перехоплення (true): гасимо click раніше, ніж він дійде до кнопки
  island.addEventListener(
    "click",
    (event) => {
      if (!swallowClick) return;
      swallowClick = false;
      event.stopPropagation();
      event.preventDefault();
    },
    true,
  );

  function endDrag() {
    dragging = false;
    document.body.classList.remove("is-dragging");
    expander.resume();
  }

  listen("island:drag-end", endDrag);

  // Вгорі чи внизу і з якого боку текст пісні — це вирішує Rust
  function applyPlacement(placement) {
    if (!placement) return;
    document.body.classList.toggle("edge-bottom", placement.edge === "bottom");
    document.body.classList.toggle("lyrics-right", placement.lyricsSide === "right");
  }
  listen("island:placement", (event) => applyPlacement(event.payload));
  invoke("island_placement").then(applyPlacement);
}
