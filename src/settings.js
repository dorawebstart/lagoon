// Вікно налаштувань: мова, колір острова, підключення Claude Code.
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { setLanguage, t } from "./i18n.js";
import { loadSettings, saveSettings } from "./settings-store.js";
import { buildHooksConfig } from "./hooks-config.js";

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

function flash(text) {
  status.textContent = text;
  status.style.opacity = "1";
  clearTimeout(statusTimer);
  statusTimer = setTimeout(() => (status.style.opacity = "0"), 1600);
}

async function save(patch) {
  await saveSettings(patch);
  flash(t("settings.saved"));
}

/** Копіює текст у буфер обміну (із запасним способом для старих WebView). */
async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const area = document.createElement("textarea");
    area.value = text;
    document.body.append(area);
    area.select();
    document.execCommand("copy");
    area.remove();
  }
}

/** Секція Claude Code: порт, токен (прихований) і кнопка «Скопіювати конфіг». */
async function setupClaudeSection() {
  const { port, token } = await invoke("agent_info");
  const tokenEl = document.querySelector("#agent-token");
  const toggle = document.querySelector("#toggle-token");
  let visible = false;

  document.querySelector("#agent-port").textContent = String(port);

  toggle.addEventListener("click", () => {
    visible = !visible;
    tokenEl.textContent = visible ? token : "••••••••";
    toggle.textContent = t(visible ? "settings.hide" : "settings.show");
  });

  document.querySelector("#copy-hooks").addEventListener("click", async () => {
    await copyText(JSON.stringify(buildHooksConfig(port, token), null, 2));
    flash(t("settings.copied"));
  });
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

  await setupClaudeSection();
}

init();
