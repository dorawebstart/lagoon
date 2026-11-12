// «Крапля»: на подію (нова пісня, «Готово ✓» від Claude Code) від острова
// відривається крапля і падає всередину екрана (острів унизу — летить угору).
//
// Як зроблено «рідкий» вигляд: за островом лежить шар із чорною копією
// острова і краплею. На цей шар на час анімації вмикаємо SVG-фільтр «goo»
// (розмиття + «стискання» прозорості назад у різкий край): поки крапля
// близько до острова, їхні краї зливаються в рідкий місток, а потім рвуться.
// Розмиття дороге без відеокарти, тож фільтр діє лише ~0,9 с і лише на тлі —
// текст, кнопки й хвиля острова малюються поверх без нього.
import { listen } from "@tauri-apps/api/event";

/** Скільки триває крапля (має збігатися з анімацією в island.css). */
const DRIP_MS = 900;

/**
 * @param {HTMLElement} island
 * @param {HTMLElement} stage  спільний контейнер островів (від нього рахуємо позицію)
 */
export function setupDrip(island, stage) {
  const layer = document.createElement("div");
  layer.className = "drip-layer";
  layer.setAttribute("aria-hidden", "true");
  const ghost = document.createElement("div");
  ghost.className = "drip-ghost";
  const drop = document.createElement("div");
  drop.className = "drip-drop";
  layer.append(ghost, drop);
  // На початок «стейджу», щоб шар лежав під островом
  stage.prepend(layer);

  const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
  let timer = 0;

  function drip() {
    if (reducedMotion.matches || timer) return;

    // Копія острова там само і такого ж розміру (offset* — відносно «стейджу»)
    const { offsetLeft: left, offsetTop: top, offsetWidth: width, offsetHeight: height } = island;
    Object.assign(ghost.style, {
      left: `${left}px`,
      top: `${top}px`,
      width: `${width}px`,
      height: `${height}px`,
      borderRadius: getComputedStyle(island).borderRadius,
    });
    // Крапля стартує з краю, що дивиться всередину екрана
    const bottom = document.body.classList.contains("edge-bottom");
    drop.style.left = `${left + width / 2 - 7}px`;
    drop.style.top = bottom ? `${top - 2}px` : `${top + height - 12}px`;

    stage.classList.add("is-dripping");
    timer = setTimeout(() => {
      stage.classList.remove("is-dripping");
      timer = 0;
    }, DRIP_MS);
  }

  // Нова пісня (не перший запуск і не та сама пісня після паузи)
  let song = null;
  listen("media:update", (event) => {
    const state = event.payload;
    const next = state?.title ? `${state.artist}\u001f${state.title}` : null;
    if (song && next && next !== song) drip();
    if (next) song = next;
  });

  // Claude Code закінчив роботу: стан острова data-agent став "done"
  let agent = island.dataset.agent;
  new MutationObserver(() => {
    if (island.dataset.agent === "done" && agent !== "done") drip();
    agent = island.dataset.agent;
  }).observe(island, { attributes: true, attributeFilter: ["data-agent"] });
}
