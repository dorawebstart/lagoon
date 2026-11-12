// Гучність програми, що грає: повзунок у розгорнутому плеєрі і коліщатко
// миші над островом. Саму гучність змінює Rust (audio.rs) через Core Audio.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Крок коліщатка: 5 % за одне «клацання». */
const WHEEL_STEP = 5;

/**
 * @typedef {object} VolumeState
 * @property {number} level  0.0–1.0
 * @property {"app" | "system"} scope  чиєю гучністю керуємо
 */

export function setupVolume(island) {
  const slider = island.querySelector("#volume");
  let appId = null;

  // Запит до Rust виконується не миттєво, а повзунок під час перетягування
  // шле десятки значень за секунду. Тому тримаємо не більше одного запиту
  // «в дорозі», а останнє значення відправляємо, щойно попередній запит завершився.
  let inFlight = false;
  /** @type {number | null} */
  let pending = null;

  /** @param {VolumeState} state */
  function show(state) {
    island.classList.add("has-volume");
    island.classList.toggle("volume-system", state.scope === "system");
    // Поки користувач тягне повзунок, не «смикаємо» його назад відповіддю з Rust
    if (pending === null) setSlider(Math.round(state.level * 100));
  }

  /** @param {number} percent */
  function setSlider(percent) {
    slider.value = String(percent);
    // CSS-змінна для заливки доріжки зліва від повзунка
    slider.style.setProperty("--fill", `${percent}%`);
  }

  async function refresh() {
    try {
      show(await invoke("audio_volume"));
    } catch (error) {
      // Немає пристрою виводу звуку тощо — просто ховаємо повзунок
      island.classList.remove("has-volume");
      console.warn("audio_volume:", error);
    }
  }

  /** @param {number} percent 0–100 */
  async function setVolume(percent) {
    pending = Math.min(100, Math.max(0, percent));
    setSlider(pending);
    if (inFlight) return;

    inFlight = true;
    while (pending !== null) {
      const level = pending / 100;
      pending = null;
      try {
        show(await invoke("audio_set_volume", { level }));
      } catch (error) {
        console.warn("audio_set_volume:", error);
      }
    }
    inFlight = false;
  }

  // Повзунок: подія input приходить на кожен рух, а не лише при відпусканні
  slider.addEventListener("input", () => setVolume(Number(slider.value)));

  // Коліщатко над островом: вгору — гучніше, вниз — тихіше
  island.addEventListener(
    "wheel",
    (event) => {
      if (!island.classList.contains("has-media") || !island.classList.contains("has-volume")) return;
      event.preventDefault();
      const direction = event.deltaY < 0 ? 1 : -1;
      setVolume(Number(slider.value) + direction * WHEEL_STEP);
    },
    // passive: false — інакше браузер не дозволить preventDefault()
    { passive: false },
  );

  // Гучність могли змінити в мікшері Windows чи в самому плеєрі,
  // тому оновлюємо її щоразу, коли острів розгортається.
  // MutationObserver спрацьовує на будь-яку зміну класів, тож запам'ятовуємо
  // попередній стан і реагуємо лише на момент розгортання.
  let expanded = false;
  new MutationObserver(() => {
    const now = island.classList.contains("is-expanded");
    if (now && !expanded) refresh();
    expanded = now;
  }).observe(island, { attributes: true, attributeFilter: ["class"] });

  // Змінилася програма-плеєр (Spotify → браузер) — шукаємо її аудіосесію заново
  listen("media:update", (event) => {
    const nextAppId = event.payload?.appId ?? null;
    if (nextAppId !== appId) {
      appId = nextAppId;
      if (appId) refresh();
    }
  });
  refresh();
}
