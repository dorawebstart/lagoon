// Повідомляє Rust, де на екрані пігулка. Поза цією зоною кліки
// проходять крізь вікно до програм під ним (див. window.rs).
import { invoke } from "@tauri-apps/api/core";

/**
 * Стежить за розміром елемента і щоразу надсилає його прямокутник у Rust.
 * Пізніше, коли пігулка розкриватиметься (крок 4), зона оновиться сама.
 */
export function trackHitRegion(element) {
  const send = () => {
    const { x, y, width, height } = element.getBoundingClientRect();
    invoke("set_hit_region", { rect: { x, y, width, height } }).catch((error) =>
      console.error("set_hit_region:", error),
    );
  };

  new ResizeObserver(send).observe(element);
  send();
}
