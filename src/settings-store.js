// Спільна робота з налаштуваннями для обох вікон (острів і налаштування).
// Дані лежать у settings.json (tauri-plugin-store); Rust читає той самий файл.
import { load } from "@tauri-apps/plugin-store";
import { emit, listen } from "@tauri-apps/api/event";

/** Має збігатися з settings::CHANGED_EVENT у src-tauri/src/settings.rs */
const CHANGED_EVENT = "settings:changed";

export const DEFAULTS = {
  language: "uk",
  islandColor: "#000000",
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
  return result;
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
