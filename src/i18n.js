// Переклади інтерфейсу. Тексти лежать у src/locales/*.json.
//
// У HTML: <span data-i18n="settings.language"></span> — текст підставиться сам.
// У JS:   t("settings.language")
import uk from "./locales/uk.json";
import en from "./locales/en.json";

export const LANGUAGES = { uk, en };
export const DEFAULT_LANGUAGE = "uk";

let current = DEFAULT_LANGUAGE;

/** Поточна мова ("uk" | "en"). */
export function getLanguage() {
  return current;
}

/** Локаль для Intl (дати, час): "uk-UA" або "en-GB". */
export function getLocale() {
  return LANGUAGES[current].locale;
}

/** Перекладає ключ на кшталт "settings.title". Якщо ключа немає — повертає сам ключ. */
export function t(key) {
  const value = key
    .split(".")
    .reduce((node, part) => (node == null ? undefined : node[part]), LANGUAGES[current]);
  return typeof value === "string" ? value : key;
}

/** Змінює мову і перекладає всі елементи з атрибутом data-i18n на сторінці. */
export function setLanguage(language) {
  current = language in LANGUAGES ? language : DEFAULT_LANGUAGE;
  document.documentElement.lang = current;
  for (const el of document.querySelectorAll("[data-i18n]")) {
    el.textContent = t(el.dataset.i18n);
  }
}
