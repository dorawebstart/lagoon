// Батарея на острові (дані — з Rust, battery.rs, подія "battery:update").
//
// - Пігулка: іконка й відсоток, коли нічого не грає (як і погода).
// - Сповіщення на 3 с (пігулка ширшає і показує два рядки), коли:
//   під'єднали / від'єднали зарядку, заряд від батареї впав до 20 % чи 10 %.
// - Плитка «Батарея» в панелі погоди: відсоток і час.
// Немає батареї (стаціонарний ПК) — нічого з цього не показується.
//
// Класи на острові: has-battery, has-notice (і data-notice — вид сповіщення).
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { t } from "../i18n.js";

/**
 * @typedef {object} BatteryInfo
 * @property {number} percent
 * @property {boolean} plugged
 * @property {boolean} charging
 * @property {number | null} minutes
 */

/** Скільки секунд показувати сповіщення. */
const NOTICE_MS = 3000;
/** Пороги низького заряду. */
const LOW_LEVELS = [20, 10];

/** 251 → "4 год 11 хв", 48 → "48 хв". */
function formatDuration(minutes) {
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h === 0) return t("battery.min").replace("{m}", m);
  return t("battery.hourMin").replace("{h}", h).replace("{m}", m);
}

/** Другий рядок: «ще 4 год 11 хв», «до повного 1 год 20 хв», «від мережі». */
function timeText(info) {
  if (info.plugged && !info.charging) return t("battery.onPower");
  if (info.minutes == null) return t("battery.estimating");
  // Шаблони з {d}: «ще {d}» / «{d} left» — порядок слів у мовах різний
  const template = t(info.charging ? "battery.toFull" : "battery.left");
  return template.replace("{d}", formatDuration(info.minutes));
}

/** Стан для кольору: заряджається (зелений), низький (червоний) чи звичайний. */
function levelOf(info) {
  if (info.charging) return "charging";
  if (!info.plugged && info.percent <= LOW_LEVELS[0]) return "low";
  return "normal";
}

/** Іконка батареї: заповнена на свій відсоток, на зарядці — з блискавкою. */
function batterySvg(info) {
  const width = Math.max(1.5, (info.percent / 100) * 14);
  // Блискавка: спершу товста лінія кольору фону (обводка), поверх — тонка світла
  const boltPath = "M12.5 6.5l-3 5.5h3.5l-3 5.5";
  const bolt = info.plugged
    ? `<path class="battery-bolt-outline" d="${boltPath}" /><path class="battery-bolt" d="${boltPath}" />`
    : "";
  return `<svg viewBox="0 0 24 24" class="battery-icon battery-icon--${levelOf(info)}">
    <rect class="battery-shell" x="2.5" y="7" width="17" height="10" rx="2.5" />
    <path class="battery-shell" d="M21.5 10.5v3" />
    <rect class="battery-fill" x="4" y="8.5" width="${width.toFixed(1)}" height="7" rx="1.2" />
    ${bolt}
  </svg>`;
}

export function setupBattery(island) {
  const mini = island.querySelector("#mini-battery");
  const notice = island.querySelector("#notice");
  const tile = island.querySelector("#battery-tile");

  /** @type {BatteryInfo | null} */
  let info = null;
  /** Попередній стан — щоб помітити зміни (зарядку під'єднали, заряд упав). */
  let previous = null;
  /** Вид сповіщення, що зараз на екрані ("plug" | "unplug" | "low"), або null. */
  let noticeKind = null;
  let noticeTimer = null;

  function renderNotice() {
    if (!noticeKind || !info) return;
    const title = {
      plug: info.charging ? t("battery.charging") : t("battery.onPower"),
      unplug: t("battery.onBattery"),
      low: t("battery.low"),
    }[noticeKind];
    notice.innerHTML = `${batterySvg(info)}
      <div class="notice__text">
        <div class="notice__title">${title} · ${info.percent} %</div>
        <div class="notice__sub">${timeText(info)}</div>
      </div>`;
  }

  function showNotice(kind) {
    noticeKind = kind;
    island.dataset.notice = kind;
    island.classList.add("has-notice");
    renderNotice();
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => {
      noticeKind = null;
      island.classList.remove("has-notice");
    }, NOTICE_MS);
  }

  /** Що змінилося з минулого разу — і чи варто про це сповістити. */
  function noticeFor(before, now) {
    if (!before) return null; // перший стан після запуску — без сповіщення
    if (before.plugged !== now.plugged) return now.plugged ? "plug" : "unplug";
    if (!now.plugged && LOW_LEVELS.some((level) => before.percent > level && now.percent <= level)) {
      return "low";
    }
    return null;
  }

  function render() {
    island.classList.toggle("has-battery", info !== null);
    if (!info) return;
    mini.innerHTML = `${batterySvg(info)}<span>${info.percent} %</span>`;
    tile.querySelector("[data-battery-icon]").innerHTML = batterySvg(info);
    const value = tile.querySelector("[data-battery-percent]");
    value.textContent = `${info.percent} %`;
    value.dataset.level = levelOf(info);
    tile.querySelector("[data-battery-time]").textContent = timeText(info);
    // Сповіщення вже на екрані — оновлюємо (наприклад, Windows порахувала час)
    renderNotice();
  }

  function update(next) {
    info = next;
    const kind = next ? noticeFor(previous, next) : null;
    previous = next;
    render();
    if (kind) showNotice(kind);
  }

  listen("battery:update", (event) => update(event.payload));
  invoke("battery_current")
    .then(update)
    .catch((error) => console.warn("battery_current:", error));

  return {
    /** Змінилася мова — перекласти «ще», «до повного», «год / хв». */
    refresh: render,
  };
}
