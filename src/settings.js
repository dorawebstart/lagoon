// Вікно налаштувань: мова і колір острова.
import { getCurrentWindow } from "@tauri-apps/api/window";
import { setLanguage, t } from "./i18n.js";
import { loadSettings, saveSettings } from "./settings-store.js";

const languageInputs = document.querySelectorAll('input[name="language"]');
const colorInputs = document.querySelectorAll('input[name="color"]');
const customColor = document.querySelector("#custom-color");
const customSwatch = customColor.closest(".swatch");
const status = document.querySelector("#status");

let statusTimer = null;

/** Перекладає сторінку і заголовок вікна. */
function applyLanguage(language) {
  setLanguage(language);
  getCurrentWindow()
    .setTitle(t("settings.title"))
    .catch(() => {});
}

/** Позначає вибраний колір: один з готових або «свій». */
function showColor(color) {
  let matched = false;
  for (const input of colorInputs) {
    input.checked = input.value.toLowerCase() === color.toLowerCase();
    matched ||= input.checked;
  }
  customSwatch.classList.toggle("is-active", !matched);
  if (!matched) customColor.value = color;
}

async function save(patch) {
  await saveSettings(patch);
  status.textContent = t("settings.saved");
  status.style.opacity = "1";
  clearTimeout(statusTimer);
  statusTimer = setTimeout(() => (status.style.opacity = "0"), 1200);
}

async function init() {
  const settings = await loadSettings();

  applyLanguage(settings.language);
  for (const input of languageInputs) input.checked = input.value === settings.language;
  showColor(settings.islandColor);

  for (const input of languageInputs) {
    input.addEventListener("change", () => {
      applyLanguage(input.value);
      save({ language: input.value });
    });
  }

  for (const input of colorInputs) {
    input.addEventListener("change", () => {
      showColor(input.value);
      save({ islandColor: input.value });
    });
  }

  // "input" спрацьовує під час вибору в палітрі, "change" — коли палітру закрили.
  // Зберігаємо лише фінальний колір, щоб не писати файл на кожен рух миші.
  customColor.addEventListener("input", () => showColor(customColor.value));
  customColor.addEventListener("change", () => save({ islandColor: customColor.value }));
}

init();
