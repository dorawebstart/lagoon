// Погода на острові (дані — з Rust, weather.rs, подія "weather:update").
//
// - Згорнута пігулка: іконка й температура поруч з годинником, лише коли
//   нічого не грає (під час музики пігулка як була).
// - Розкритий острів: панель «Погода» — температура, опис, місто і плитки
//   (вітер з компасом, вологість, схід і захід сонця). Якщо грає музика —
//   угорі вкладки «Плеєр / Погода»; без музики там дата, а панель погоди
//   замінює великий годинник.
//
// Класи на острові (кольори й розміри — island.css):
//   has-weather  — є свіжа погода;
//   has-tabs     — є і погода, і музика: показуємо вкладки;
//   show-weather — у розкритому острові зараз панель погоди.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { t, getLocale } from "../i18n.js";

/**
 * @typedef {object} Weather
 * @property {string} city
 * @property {number} temperature
 * @property {number} apparentTemperature
 * @property {number} weatherCode
 * @property {boolean} isDay
 * @property {number} windSpeed      м/с
 * @property {number} windDirection  звідки дме, градуси (0 — північ)
 * @property {number} humidity       %
 * @property {string} sunrise        "HH:MM"
 * @property {string} sunset         "HH:MM"
 * @property {number} updatedAt      секунди від 1970 року
 */

/** Погоду, старшу за 3 години, не показуємо (так само вирішує і Rust). */
const MAX_AGE_S = 3 * 60 * 60;

/** Код погоди WMO → вид іконки (і ключ опису в locales: weather.kinds.*). */
function kindOf(code) {
  if (code === 0) return "clear";
  if (code <= 2) return "partly";
  if (code === 3) return "cloudy";
  if (code === 45 || code === 48) return "fog";
  if (code >= 51 && code <= 57) return "drizzle";
  if ((code >= 61 && code <= 67) || (code >= 80 && code <= 82)) return "rain";
  if ((code >= 71 && code <= 77) || code === 85 || code === 86) return "snow";
  if (code >= 95) return "thunder";
  return "cloudy";
}

// Іконки — прості лінії (stroke) кольором тексту; сонце й краплі — акцентні.
const SUN = '<circle cx="12" cy="12" r="4"/><path d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M5.3 18.7l1.4-1.4M17.3 6.7l1.4-1.4"/>';
const MOON = '<path d="M19 14.5A7.5 7.5 0 0 1 9.5 5a7.5 7.5 0 1 0 9.5 9.5z"/>';
const CLOUD = '<path d="M7 18h10a4 4 0 0 0 .6-7.95A5.5 5.5 0 0 0 7 11a3.5 3.5 0 0 0 0 7z"/>';
const SMALL_CLOUD = '<path d="M9 19h8.5a3.5 3.5 0 0 0 .5-6.96A4.8 4.8 0 0 0 9 13a3 3 0 0 0 0 6z"/>';
/** Хмара вище — під нею місце для дощу, снігу чи блискавки. */
const HIGH_CLOUD = '<path d="M7 14h10a4 4 0 0 0 .6-7.95A5.5 5.5 0 0 0 7 7a3.5 3.5 0 0 0 0 7z"/>';
const ICONS = {
  clear: (day) => (day ? SUN : MOON),
  partly: (day) =>
    (day
      ? '<circle cx="8.5" cy="8.5" r="3"/><path d="M8.5 2.5v1.5M2.5 8.5H4M4.3 4.3l1 1M12.7 4.3l-1 1"/>'
      : '<path d="M12 9.5A4.5 4.5 0 0 1 6.5 4a4.5 4.5 0 1 0 5.5 5.5z"/>') + SMALL_CLOUD,
  cloudy: () => CLOUD,
  fog: () => '<path d="M4 9h16M6 13h12M4 17h16"/>',
  drizzle: () => HIGH_CLOUD + '<path d="M9 17.5v1M12 19v1M15 17.5v1"/>',
  rain: () => HIGH_CLOUD + '<path d="M8 17l-1 3M12 17l-1 3M16 17l-1 3"/>',
  snow: () => HIGH_CLOUD + '<path d="M8 18h.01M12 20h.01M16 18h.01M10 21.5h.01M14 21.5h.01"/>',
  thunder: () => HIGH_CLOUD + '<path d="M12.5 14l-2 3.5h3l-2 3.5"/>',
};

/** SVG-іконка погоди. */
function iconSvg(weather) {
  const kind = kindOf(weather.weatherCode);
  return `<svg viewBox="0 0 24 24" class="weather-icon weather-icon--${kind}">${ICONS[kind](weather.isDay)}</svg>`;
}

/** 7.4 → "+7°", -2.6 → "−3°" (справжній мінус, не дефіс), 0 → "0°". */
function formatTemperature(value) {
  const rounded = Math.round(value);
  if (rounded > 0) return `+${rounded}°`;
  if (rounded < 0) return `−${Math.abs(rounded)}°`;
  return "0°";
}

/** Напрямок «звідки дме» → скорочення з 8 сторін світу (Пн, ПнСх…). */
function compassPoint(degrees) {
  const index = Math.round((((degrees % 360) + 360) % 360) / 45) % 8;
  return t(`weather.compass.${index}`);
}

export function setupWeather(island) {
  const mini = island.querySelector("#mini-weather");
  const panel = island.querySelector("#weather");
  const tabs = island.querySelectorAll("[data-tab]");

  /** @type {Weather | null} */
  let weather = null;
  /** Вибрана вкладка, коли грає музика: "player" | "weather". */
  let tab = "player";

  function isFresh(value) {
    return value !== null && Date.now() / 1000 - value.updatedAt < MAX_AGE_S;
  }

  /** Хто зараз видно в розкритому острові — і класи для CSS. */
  function syncView() {
    const hasWeather = isFresh(weather);
    const hasMedia = island.classList.contains("has-media");
    island.classList.toggle("has-weather", hasWeather);
    island.classList.toggle("has-tabs", hasWeather && hasMedia);
    // Без музики — одразу погода; з музикою — та вкладка, яку вибрали
    island.classList.toggle("show-weather", hasWeather && (!hasMedia || tab === "weather"));
    for (const button of tabs) {
      button.setAttribute("aria-selected", String(button.dataset.tab === tab));
    }
  }

  function render() {
    syncView();
    if (!isFresh(weather)) return;
    const w = weather;

    mini.innerHTML = `${iconSvg(w)}<span>${formatTemperature(w.temperature)}</span>`;

    const updated = new Intl.DateTimeFormat(getLocale(), { hour: "2-digit", minute: "2-digit" }).format(
      new Date(w.updatedAt * 1000),
    );
    panel.querySelector("[data-weather-icon]").innerHTML = iconSvg(w);
    panel.querySelector("[data-weather-temp]").textContent = formatTemperature(w.temperature);
    panel.querySelector("[data-weather-desc]").textContent =
      `${t(`weather.kinds.${kindOf(w.weatherCode)}`)} · ${t("weather.feelsLike")} ${formatTemperature(w.apparentTemperature)}`;
    panel.querySelector("[data-weather-city]").textContent = w.city;
    panel.querySelector("[data-weather-updated]").textContent = `${t("weather.updated")} ${updated}`;

    // Стрілка компаса показує, КУДИ дме вітер: протилежно до «звідки»
    panel.querySelector("[data-wind-arrow]").style.transform = `rotate(${w.windDirection + 180}deg)`;
    panel.querySelector("[data-wind]").textContent =
      `${Math.round(w.windSpeed)} ${t("weather.ms")} · ${compassPoint(w.windDirection)}`;
    panel.querySelector("[data-humidity]").textContent = `${Math.round(w.humidity)} %`;
    panel.querySelector("[data-sunrise]").textContent = w.sunrise;
    panel.querySelector("[data-sunset]").textContent = w.sunset;
  }

  for (const button of tabs) {
    button.addEventListener("click", () => {
      tab = button.dataset.tab;
      syncView();
    });
  }

  // Острів згорнули — наступного разу знову відкриється плеєр.
  // Музика з'явилась / зникла (клас has-media ставить player.js) — перерахувати вкладки.
  new MutationObserver(() => {
    if (!island.classList.contains("is-expanded")) tab = "player";
    syncView();
  }).observe(island, { attributes: true, attributeFilter: ["class"] });

  listen("weather:update", (event) => {
    weather = event.payload;
    render();
  });
  // Погода (з кешу) могла бути ще до того, як сторінка підписалась
  invoke("weather_current")
    .then((value) => {
      weather = value;
      render();
    })
    .catch((error) => console.warn("weather_current:", error));

  // Без інтернету Rust сам скаже сховати застарілу погоду, а це — запасна
  // перевірка раз на 10 хв (дешево: лише порівняння часу)
  setInterval(syncView, 10 * 60 * 1000);

  return {
    /** Змінилася мова — перекласти опис, сторони світу й «оновлено». */
    refresh: render,
  };
}
