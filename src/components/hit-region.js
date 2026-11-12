// Повідомляє Rust, де на екрані пігулка. Поза цією зоною кліки
// проходять крізь вікно до програм під ним (див. window.rs).
import { invoke } from "@tauri-apps/api/core";

/**
 * Стежить за розміром елемента і щоразу надсилає його прямокутник у Rust.
 * Зона оновлюється, коли пігулка змінює розмір або переїжджає у вікні.
 */
export function trackHitRegion(element) {
  const send = () => {
    const { x, y, width, height } = element.getBoundingClientRect();
    invoke("set_hit_region", { rect: { x, y, width, height } }).catch((error) =>
      console.error("set_hit_region:", error),
    );
  };

  new ResizeObserver(send).observe(element);
  // ResizeObserver бачить лише зміну розміру. А коли острів переїжджає всередині
  // вікна (вниз, до лівого чи правого краю), розмір той самий — змінюються лише
  // класи на <body> (edge-bottom, align-left…, див. drag.js). Стежимо за ними
  // і надсилаємо прямокутник на наступному кадрі, коли CSS уже застосовано.
  const resend = () => requestAnimationFrame(send);
  new MutationObserver(resend).observe(document.body, {
    attributes: true,
    attributeFilter: ["class"],
  });
  // Масштаб (--scale) main.js ставить у style на <html>: острів стає більшим,
  // але ResizeObserver цього не бачить (transform не змінює розміру елемента)
  new MutationObserver(resend).observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["style"],
  });
  send();
}
