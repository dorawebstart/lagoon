// Готові палітри: фон острова, акцент і колір тексту одним кліком.
// Назви — у src/locales/*.json (ключі palettes.<id>).

/**
 * @typedef {object} Palette
 * @property {string} id      ключ для перекладу назви
 * @property {string} bg      фон острова (тема «Свій колір»)
 * @property {string} accent  еквалайзер, прогрес, гучність, поточний рядок
 * @property {string} text    основний текст
 */

/** @type {Palette[]} */
export const PALETTES = [
  { id: "ocean", bg: "#0b2a3c", accent: "#5ec8e5", text: "#e6f6fb" },
  { id: "sunset", bg: "#2a1418", accent: "#ff8a5c", text: "#ffe9dc" },
  { id: "forest", bg: "#10241a", accent: "#7fd18b", text: "#e8f5ea" },
  { id: "lavender", bg: "#221b33", accent: "#b7a3ff", text: "#f0ebff" },
];
