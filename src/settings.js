// Вікно налаштувань: мова, тема острова, позиція, підключення Claude Code.
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { setLanguage, t } from "./i18n.js";
import { loadSettings, saveSettings } from "./settings-store.js";
import { buildHooksConfig } from "./hooks-config.js";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";

const languageInputs = document.querySelectorAll('input[name="language"]');
const themeInputs = document.querySelectorAll('input[name="theme"]');
const swatches = document.querySelector("#swatches");
const colorInputs = document.querySelectorAll('input[name="color"]');
const edgeInputs = document.querySelectorAll('input[name="edge"]');
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

/** Позначає вибрану тему; кольори показуємо лише для теми «Свій колір». */
function showTheme(theme) {
  for (const input of themeInputs) input.checked = input.value === theme;
  swatches.hidden = theme !== "custom";
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
  showTheme(settings.theme);
  showColor(settings.islandColor);

  for (const input of languageInputs) {
    input.addEventListener("change", () => {
      applyLanguage(input.value);
      save({ language: input.value });
    });
  }

  for (const input of themeInputs) {
    input.addEventListener("change", () => {
      showTheme(input.value);
      save({ theme: input.value });
    });
  }

  for (const input of colorInputs) {
    input.addEventListener("change", () => {
      showColor(input.value);
      save({ theme: "custom", islandColor: input.value });
    });
  }

  // "input" спрацьовує під час вибору в палітрі, "change" — коли палітру закрили.
  // Зберігаємо лише фінальний колір, щоб не писати файл на кожен рух миші.
  customColor.addEventListener("input", () => showColor(customColor.value));
  customColor.addEventListener("change", () => save({ theme: "custom", islandColor: customColor.value }));

  const coverAccent = document.querySelector("#cover-accent");
  coverAccent.checked = settings.coverAccent;
  coverAccent.addEventListener("change", () => save({ coverAccent: coverAccent.checked }));

  setupPosition(settings);
  await setupAutostart();
  await setupClaudeSection();
}

/**
 * Позиція острова: вгорі / внизу і «Скинути позицію».
 * Саме вікно переставляє Rust, коли отримує подію settings:changed.
 */
function setupPosition(settings) {
  const showEdge = (edge) => {
    for (const input of edgeInputs) input.checked = input.value === edge;
  };
  showEdge(settings.islandEdge);

  for (const input of edgeInputs) {
    // Зсув лишаємо той самий: острів перескакує на інший край у тому ж місці
    input.addEventListener("change", () => save({ islandEdge: input.value }));
  }

  document.querySelector("#reset-position").addEventListener("click", () => {
    showEdge("top");
    save({ islandEdge: "top", islandAlign: "center", islandOffset: 0 });
  });
}

/**
 * Автозапуск: стан беремо з самої Windows (запис у реєстрі), а не з settings.json,
 * щоб перемикач завжди показував правду.
 */
async function setupAutostart() {
  const toggle = document.querySelector("#autostart");
  toggle.checked = await isEnabled().catch(() => false);
  toggle.addEventListener("change", async () => {
    try {
      if (toggle.checked) await enable();
      else await disable();
      flash(t("settings.saved"));
    } catch (error) {
      console.warn("autostart:", error);
      toggle.checked = !toggle.checked; // не вдалося — повертаємо як було
    }
  });
}

init();
