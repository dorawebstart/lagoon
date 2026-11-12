// Спільна робота з налаштуваннями для обох вікон (острів і налаштування).
// Дані лежать у settings.json (tauri-plugin-store); Rust читає той самий файл.
import { load } from "@tauri-apps/plugin-store";
import { emit, listen } from "@tauri-apps/api/event";

/** Має збігатися з settings::CHANGED_EVENT у src-tauri/src/settings.rs */
const CHANGED_EVENT = "settings:changed";

export const DEFAULTS = {
  language: "uk",
  /** Тема острова: "classic" | "graphite" | "glass" | "custom" (свій колір — islandColor). */
  theme: "classic",
  islandColor: "#000000",
  /** Акцент (еквалайзер, прогрес, гучність, поточний рядок): "cover" | "custom" | "theme". */
  accentMode: "cover",
  accentColor: "#7ee787",
  /** Свій колір тексту; порожній рядок — стандартний (білий). */
  textColor: "",
  /** Розміри: масштаб острова (%), ширина (px) і шрифт (px) тексту пісні. */
  islandScale: 100,
  lyricsWidth: 280,
  lyricsFont: 12,
  /** Показувати острівець з текстом пісні (кнопка 🎤 у плеєрі). */
  showLyrics: true,
  /** Позиція острова: край ("top" | "bottom"), прив'язка ("left" | "center" | "right")
      і зсув точки прив'язки у частках ширини екрана. */
  islandEdge: "top",
  islandAlign: "center",
  islandOffset: 0,
};

let storePromise = null;
function getStore() {
  // Відкриваємо файл один раз і далі використовуємо той самий об'єкт
  storePromise ??= load("settings.json", { defaults: DEFAULTS, autoSave: false });
  return storePromise;
}

/** Повертає всі налаштування (зі значеннями за замовчуванням для відсутніх). */
export async function loadSettings() {
  const store = await getStore();
  const result = { ...DEFAULTS };
  for (const key of Object.keys(DEFAULTS)) {
    const value = await store.get(key);
    if (value !== undefined && value !== null) result[key] = value;
  }
  // До 0.3 тем не було, лише колір. Якщо тему ще не зберігали, виводимо її
  // з кольору, щоб у тих, хто оновився, острів лишився таким самим.
  if ((await store.get("theme")) == null) {
    result.theme = themeFromColor(result.islandColor);
  }
  // У першій версії кроку 17 був лише перемикач coverAccent (так / ні).
  if ((await store.get("accentMode")) == null && (await store.get("coverAccent")) === false) {
    result.accentMode = "theme";
  }
  return result;
}

/** Старий колір острова → тема: чорний — «Класична», графіт — «Графіт», інше — «Свій колір». */
function themeFromColor(color) {
  const value = String(color).toLowerCase();
  if (value === "#000000") return "classic";
  if (value === "#2c2c2e") return "graphite";
  return "custom";
}

/**
 * Зберігає зміни (наприклад `{ language: "en" }`) і повідомляє всі вікна та Rust.
 * Повертає повний набір налаштувань після зміни.
 */
export async function saveSettings(patch) {
  const store = await getStore();
  for (const [key, value] of Object.entries(patch)) {
    await store.set(key, value);
  }
  await store.save();
  const settings = await loadSettings();
  await emit(CHANGED_EVENT, settings);
  return settings;
}

/** Викликає callback(settings) щоразу, коли налаштування змінено в будь-якому вікні. */
export function onSettingsChanged(callback) {
  return listen(CHANGED_EVENT, (event) => callback(event.payload));
}
