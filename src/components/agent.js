// Claude Code в острові: події приходять з Rust (agent.rs).
//
// Що показуємо:
//  - працює            → у пігулці пульсує крапка і «Працюю…» або «Bash: git status»;
//  - чекає на тебе     → пігулка світиться (Notification);
//  - готово            → «Готово ✓» на кілька секунд (Stop);
//  - запит дозволу     → острів розкривається сам і показує кнопки
//                        «Дозволити / Відхилити / У терміналі».
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { t } from "../i18n.js";

/**
 * @typedef {object} AgentEvent
 * @property {string} kind      "UserPromptSubmit" | "PreToolUse" | "Notification" | "Stop" | ...
 * @property {string} sessionId
 * @property {string} project
 * @property {string | null} tool
 * @property {string | null} detail
 */

/**
 * @typedef {object} PermissionAsk
 * @property {string} id
 * @property {string} sessionId
 * @property {string} project
 * @property {string} tool
 * @property {string} detail
 */

/**
 * Захист від випадкового рішення: після появи нового запиту кнопки
 * не реагують стільки мс. Інакше клік «Дозволити» для попереднього запиту
 * міг би схвалити і наступний, що з'явився під курсором.
 */
const ARM_DELAY_MS = 500;

/** Скільки показувати «Готово ✓». */
const DONE_MS = 4000;
/** Якщо від Claude Code довго немає подій — вважаємо, що він уже не працює. */
const STALE_MS = 5 * 60 * 1000;

/**
 * @param {HTMLElement} island
 * @param {{ pin: () => void, unpin: () => void }} expander
 */
export function setupAgent(island, expander) {
  const statusText = island.querySelector("#agent-status");
  const askTitle = island.querySelector("#ask-title");
  const askProject = island.querySelector("#ask-project");
  const askDetail = island.querySelector("#ask-detail");

  /** Стан: "idle" | "working" | "attention" | "done" */
  let mode = "idle";
  let label = "";
  let resetTimer = null;

  /** @type {PermissionAsk[]} — черга запитів (кілька сесій можуть питати одночасно) */
  const queue = [];

  // Який запит зараз показано і коли він з'явився
  let shownId = null;
  let shownAt = 0;
  // Запит, для якого користувач НАТИСНУВ кнопку (pointerdown) уже після його появи
  let pressedFor = null;

  function setMode(next, text = "", ttl = STALE_MS) {
    mode = next;
    label = text;
    clearTimeout(resetTimer);
    if (next !== "idle") resetTimer = setTimeout(() => setMode("idle"), ttl);
    render();
  }

  function render() {
    island.dataset.agent = mode;
    island.classList.toggle("has-agent", mode !== "idle");
    statusText.textContent = label;

    const ask = queue[0];
    island.classList.toggle("is-asking", Boolean(ask));
    if (ask && ask.id !== shownId) {
      // Новий запит на екрані: «роззброюємо» кнопки
      shownId = ask.id;
      shownAt = performance.now();
      pressedFor = null;
      document.activeElement?.blur();
    }
    if (!ask) shownId = null;

    if (ask) {
      askTitle.textContent = t("agent.permissionTitle").replace("{tool}", ask.tool || "?");
      askProject.textContent = ask.project;
      askDetail.textContent = ask.detail;
      expander.pin();
    } else {
      expander.unpin();
    }
  }

  /** @param {AgentEvent} event */
  function onEvent(event) {
    switch (event.kind) {
      case "UserPromptSubmit":
        setMode("working", t("agent.working"));
        break;
      case "PreToolUse":
        setMode("working", event.detail ? `${event.tool}: ${event.detail}` : event.tool || t("agent.working"));
        break;
      case "Notification":
        setMode("attention", event.detail || t("agent.waiting"));
        break;
      case "Stop":
        setMode("done", t("agent.done"), DONE_MS);
        break;
      case "SessionEnd":
        setMode("idle");
        break;
      default:
        // Інші події (PostToolUse тощо) лише продовжують «працює»
        if (mode === "working") setMode("working", label);
    }
  }

  listen("agent:event", (e) => onEvent(e.payload));

  listen("agent:permission", (e) => {
    queue.push(e.payload);
    setMode("attention", t("agent.waiting"));
  });

  // Запит закрито (відповідь, таймаут або Claude Code його скасував)
  listen("agent:permission-closed", (e) => {
    const index = queue.findIndex((ask) => ask.id === e.payload);
    if (index !== -1) queue.splice(index, 1);
    if (queue.length === 0 && mode === "attention") setMode("working", t("agent.working"));
    else render();
  });

  // Кнопки «Дозволити / Відхилити / У терміналі»
  const armed = () => performance.now() - shownAt >= ARM_DELAY_MS;

  island.querySelectorAll("[data-decision]").forEach((button) => {
    // Натискання має початися вже на поточному запиті і не одразу після його появи
    button.addEventListener("pointerdown", () => {
      pressedFor = queue[0] && armed() ? queue[0].id : null;
    });

    button.addEventListener("click", async (event) => {
      const ask = queue[0];
      if (!ask) return;
      // event.detail === 0 — «клік» з клавіатури (Enter/Space), без миші
      const fromKeyboard = event.detail === 0;
      const valid = fromKeyboard ? armed() : pressedFor === ask.id;
      pressedFor = null;
      if (!valid) return;
      button.blur();
      // Прибираємо одразу, не чекаючи події від Rust
      queue.shift();
      setMode("working", t("agent.working"));
      try {
        await invoke("agent_respond", { id: ask.id, decision: button.dataset.decision });
      } catch (error) {
        console.warn("agent_respond:", error);
      }
    });
  });

  // Мова змінилася — перемальовуємо тексти, що формуються в JS
  return { refresh: render };
}
