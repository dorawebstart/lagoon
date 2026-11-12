// Claude Code в острові: події приходять з Rust (agent.rs).
// Крок 8: лише приймаємо події і пишемо в консоль.
// Крок 9: статус на острові і кнопки «Дозволити / Відхилити».
import { listen } from "@tauri-apps/api/event";

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

export function setupAgent() {
  listen("agent:event", (event) => console.log("agent:event", event.payload));
  listen("agent:permission", (event) => console.log("agent:permission", event.payload));
  listen("agent:permission-closed", (event) => console.log("agent:permission-closed", event.payload));
}
