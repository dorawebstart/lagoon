// Вікно налаштувань: мова, тема острова, позиція, підключення Claude Code.
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { setLanguage, t } from "./i18n.js";
import { loadSettings, saveSettings } from "./settings-store.js";
import { buildHooksConfig } from "./hooks-config.js";
import { PALETTES } from "./palettes.js";
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

  setupColors(settings);
  setupSizes(settings);

  const showWave = document.querySelector("#show-wave");
  showWave.checked = settings.showWave;
  showWave.addEventListener("change", () => save({ showWave: showWave.checked }));
  setupPosition(settings);
  await setupAutostart();
  await setupClaudeSection();
}

/** Біла «стандартна» підстановка для палітри, коли свій колір тексту не вибрано. */
const DEFAULT_TEXT = "#ffffff";

/**
 * Кольори: готові палітри, акцент (обкладинка / свій / стандартний) і колір тексту.
 * Кожна зміна зберігається одразу — острів оновлюється без перезапуску.
 */
function setupColors(settings) {
  const accentInputs = document.querySelectorAll('input[name="accent"]');
  const accentRow = document.querySelector("#accent-row");
  const accentColor = document.querySelector("#accent-color");
  const textColor = document.querySelector("#text-color");

  const showAccent = (mode, color) => {
    for (const input of accentInputs) input.checked = input.value === mode;
    accentRow.hidden = mode !== "custom";
    accentColor.value = color;
  };
  const showText = (color) => {
    textColor.value = color || DEFAULT_TEXT;
  };
  showAccent(settings.accentMode, settings.accentColor);
  showText(settings.textColor);

  for (const input of accentInputs) {
    input.addEventListener("change", () => {
      showAccent(input.value, accentColor.value);
      save({ accentMode: input.value });
    });
  }
  // "change" — коли палітру кольорів закрили (а не на кожен рух миші)
  accentColor.addEventListener("change", () => save({ accentMode: "custom", accentColor: accentColor.value }));
  textColor.addEventListener("change", () => save({ textColor: textColor.value }));
  document.querySelector("#text-reset").addEventListener("click", () => {
    showText("");
    save({ textColor: "" });
  });

  // Палітри: кнопка з міні-пігулкою для кожної з src/palettes.js
  const container = document.querySelector("#palettes");
  for (const palette of PALETTES) {
    const button = document.createElement("button");
    button.className = "palette";
    button.innerHTML = `
      <span class="palette__pill" style="background:${palette.bg};color:${palette.text}">
        Aa<span class="palette__dot" style="background:${palette.accent}"></span>
      </span>
      <span data-i18n="palettes.${palette.id}">${t(`palettes.${palette.id}`)}</span>`;
    button.addEventListener("click", () => {
      showTheme("custom");
      showColor(palette.bg);
      showAccent("custom", palette.accent);
      showText(palette.text);
      save({
        theme: "custom",
        islandColor: palette.bg,
        accentMode: "custom",
        accentColor: palette.accent,
        textColor: palette.text,
      });
    });
    container.append(button);
  }
}

/** Повзунки розмірів: ключ налаштування → поле, підпис значення й одиниця. */
const SIZE_FIELDS = [
  { key: "islandScale", id: "island-scale", unit: "%" },
  { key: "lyricsWidth", id: "lyrics-width", unit: "px" },
  { key: "lyricsFont", id: "lyrics-font", unit: "px" },
];

/** Значення за замовчуванням для «Скинути розміри» — як до кроку 18. */
const DEFAULT_SIZES = { islandScale: 100, lyricsWidth: 280, lyricsFont: 12 };

/**
 * Розмір: масштаб острова, ширина й шрифт тексту пісні.
 * Під час руху повзунка ("input") оновлюємо лише підпис, а зберігаємо,
 * коли повзунок відпустили ("change"), — щоб не змінювати вікно 30 разів на секунду.
 */
function setupSizes(settings) {
  const fields = SIZE_FIELDS.map((field) => ({
    ...field,
    input: document.querySelector(`#${field.id}`),
    output: document.querySelector(`#${field.id}-out`),
  }));
  const show = (values) => {
    for (const field of fields) {
      field.input.value = values[field.key];
      field.output.textContent = `${values[field.key]} ${field.unit}`;
    }
  };
  show(settings);

  for (const field of fields) {
    field.input.addEventListener("input", () => {
      field.output.textContent = `${field.input.value} ${field.unit}`;
    });
    field.input.addEventListener("change", () => save({ [field.key]: Number(field.input.value) }));
  }

  document.querySelector("#reset-sizes").addEventListener("click", () => {
    show(DEFAULT_SIZES);
    save(DEFAULT_SIZES);
  });
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
