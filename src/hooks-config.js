// Конфіг hooks для ~/.claude/settings.json (або .claude/settings.json у проєкті).
// Документація: https://code.claude.com/docs/en/hooks
//
// Використовуємо hooks типу "http": Claude Code сам надсилає POST острову,
// тож жодних скриптів не потрібно. Якщо острів не запущено, Claude Code
// не зможе з'єднатися і просто працюватиме як звичайно.

/**
 * @param {number} port
 * @param {string} token
 * @returns {object} — об'єкт { hooks: {...} } для вставки в settings.json
 */
export function buildHooksConfig(port, token) {
  const handler = (timeout) => ({
    type: "http",
    url: `http://127.0.0.1:${port}/hook`,
    headers: { "X-Lagoon-Token": token },
    timeout,
  });

  // Звичайні події — короткий таймаут, щоб острів ніколи не гальмував Claude Code
  const quick = [{ hooks: [handler(5)] }];

  return {
    hooks: {
      UserPromptSubmit: quick,
      PreToolUse: quick,
      Notification: quick,
      Stop: quick,
      // Запит дозволу: острів чекає рішення до 55 с, тому таймаут більший
      PermissionRequest: [{ hooks: [handler(60)] }],
    },
  };
}
