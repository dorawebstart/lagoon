// «Хвиля» — тонкі смужки вздовж краю острова, що рухаються в такт музиці.
// Рівень звуку (0–1) ~30 разів на секунду надсилає Rust (meter.rs, подія
// audio:level). Це гучність, а не частоти, тому кожна смужка має власний
// «ритм» (фаза й швидкість) — так хвиля виглядає живою, а не як одна лінія.
//
// Економія процесора: кадр малюємо лише коли приходить рівень (без
// requestAnimationFrame), а Rust шле його тільки поки хвиля увімкнена і
// музика грає. Якщо в Windows вимкнено анімації — хвилю не показуємо.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Кількість смужок. */
const BARS = 28;
/** Підсилення: звичайна музика рідко доходить до 1.0. */
const GAIN = 1.6;

export function setupWave(island) {
  const wave = document.createElement("div");
  wave.className = "wave";
  wave.setAttribute("aria-hidden", "true");

  // Кожна смужка — свій «ритм»: зсув фази і швидкість коливання
  const bars = Array.from({ length: BARS }, (_, index) => {
    const element = document.createElement("i");
    wave.append(element);
    return { element, phase: index * 0.9, speed: 4 + ((index * 7) % 5) };
  });
  island.append(wave);

  const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
  let enabled = true;

  /** Показує рівень: висота кожної смужки = рівень × її власне коливання. */
  function draw(level) {
    const time = performance.now() / 1000;
    for (const bar of bars) {
      const wobble = 0.55 + 0.45 * Math.sin(time * bar.speed + bar.phase);
      const height = Math.min(1, Math.max(0.08, level * GAIN * wobble));
      bar.element.style.transform = `scaleY(${height.toFixed(2)})`;
    }
  }

  /** Повідомляє Rust, чи потрібен рівень звуку (інакше він не міряє). */
  function sync() {
    const active = enabled && !reducedMotion.matches;
    island.classList.toggle("has-wave", active);
    if (!active) draw(0);
    invoke("audio_wave", { enabled: active }).catch((error) => console.warn("audio_wave:", error));
  }

  listen("audio:level", (event) => {
    if (enabled && !reducedMotion.matches) draw(Number(event.payload) || 0);
  });
  // Користувач увімкнув / вимкнув анімації в Windows — підлаштовуємось одразу
  reducedMotion.addEventListener("change", sync);

  return {
    /** Перемикач «Хвиля в такт музиці» з налаштувань. */
    setEnabled(value) {
      enabled = value !== false;
      sync();
    },
  };
}
