// Колір з обкладинки: беремо головний яскравий колір обкладинки пісні і
// фарбуємо ним еквалайзер, прогрес-бар, повзунок гучності й поточний рядок
// тексту пісні (CSS-змінна --cover на <html>, див. island.css).
//
// Як шукаємо колір: зменшуємо обкладинку до 32×32 на canvas, розкладаємо
// пікселі по «кошиках» схожих кольорів і обираємо кошик, де багато пікселів
// і вони насичені (а не сірі, чорні чи білі).
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Розмір зменшеної копії: 32×32 = 1024 пікселі — досить, щоб угадати колір. */
const SAMPLE_SIZE = 32;

export function setupCoverAccent() {
  let enabled = true;
  let thumbnail = null;
  /** Колір останньої обкладинки (кеш: та сама картинка — той самий колір). */
  let cached = { thumbnail: null, color: null };
  /** Номер запиту — щоб застарілий результат (пісня вже змінилася) не переміг. */
  let request = 0;

  function apply(color) {
    const root = document.documentElement.style;
    if (enabled && color) root.setProperty("--cover", color);
    else root.removeProperty("--cover");
  }

  async function update() {
    if (!enabled || !thumbnail) return apply(null);
    if (cached.thumbnail === thumbnail) return apply(cached.color);

    const id = ++request;
    const color = await dominantColor(thumbnail).catch(() => null);
    if (id !== request) return; // за цей час прийшла інша обкладинка
    cached = { thumbnail, color };
    apply(color);
  }

  function onMedia(state) {
    thumbnail = state?.title ? state.thumbnail : null;
    update();
  }

  listen("media:update", (event) => onMedia(event.payload));
  invoke("media_current").then(onMedia);

  return {
    /** Перемикач «Колір з обкладинки» з налаштувань. */
    setEnabled(value) {
      enabled = value !== false;
      update();
    },
  };
}

/**
 * Головний яскравий колір картинки як рядок "hsl(…)", або null, якщо
 * картинка майже безбарвна (тоді лишається звичайний колір теми).
 */
async function dominantColor(dataUrl) {
  const image = new Image();
  image.src = dataUrl;
  // decode() чекає, поки картинка розпакується (Promise)
  await image.decode();

  const canvas = document.createElement("canvas");
  canvas.width = SAMPLE_SIZE;
  canvas.height = SAMPLE_SIZE;
  const context = canvas.getContext("2d", { willReadFrequently: true });
  context.drawImage(image, 0, 0, SAMPLE_SIZE, SAMPLE_SIZE);
  // data — плаский масив [r, g, b, a, r, g, b, a, …]
  const { data } = context.getImageData(0, 0, SAMPLE_SIZE, SAMPLE_SIZE);

  // Кошики: колір округлюємо до 8 рівнів на канал (8³ = 512 кошиків).
  // Великі кошики потрібні для плавних (пастельних) обкладинок: інакше схожі
  // відтінки розсипаються по сотнях дрібних кошиків і жоден не «виграє».
  const buckets = new Map();
  let colorful = 0;
  for (let i = 0; i < data.length; i += 4) {
    const [r, g, b, a] = [data[i], data[i + 1], data[i + 2], data[i + 3]];
    if (a < 128) continue;
    const { s, l } = toHsl(r, g, b);
    // Майже чорні, майже білі й сірі пікселі не рахуємо
    if (l < 0.12 || l > 0.92 || s < 0.2) continue;

    colorful += 1;
    // >> 5 — ділення на 32 без остачі: 256 рівнів → 8
    const key = ((r >> 5) << 6) | ((g >> 5) << 3) | (b >> 5);
    const bucket = buckets.get(key) ?? { count: 0, r: 0, g: 0, b: 0, s: 0 };
    bucket.count += 1;
    bucket.r += r;
    bucket.g += g;
    bucket.b += b;
    bucket.s += s;
    buckets.set(key, bucket);
  }

  // Кольорових пікселів менше ніж 3 % — картинка по суті сіра
  if (colorful < SAMPLE_SIZE * SAMPLE_SIZE * 0.03) return null;

  // Найкращий кошик: сума насиченостей його пікселів — тобто «багато пікселів»
  // і «яскраві» разом (сірувата більшість не переможе яскравий акцент)
  let best = null;
  let bestScore = 0;
  for (const bucket of buckets.values()) {
    const score = bucket.s;
    if (score > bestScore) {
      best = bucket;
      bestScore = score;
    }
  }
  if (!best) return null;

  const average = (sum) => Math.round(sum / best.count);
  const { h, s, l } = toHsl(average(best.r), average(best.g), average(best.b));
  // Щоб колір було видно на темному острові: не надто темний і не блідий
  const lightness = Math.min(0.75, Math.max(0.6, l));
  const saturation = Math.max(0.55, s);
  return `hsl(${Math.round(h * 360)} ${Math.round(saturation * 100)}% ${Math.round(lightness * 100)}%)`;
}

/** RGB (0–255) → HSL (усе 0–1). */
function toHsl(r, g, b) {
  r /= 255;
  g /= 255;
  b /= 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return { h: 0, s: 0, l };

  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h;
  if (max === r) h = (g - b) / d + (g < b ? 6 : 0);
  else if (max === g) h = (b - r) / d + 2;
  else h = (r - g) / d + 4;
  return { h: h / 6, s, l };
}
