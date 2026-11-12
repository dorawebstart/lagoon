# Що нового / Changelog

## v0.2.0 «Плеєр+» — 12.11.2026

### Нове
- 🔊 **Гучність програми, що грає** — повзунок у розгорнутому плеєрі і коліщатко миші над островом (±5 %). Змінюється гучність саме Spotify / браузера в мікшері Windows, а не всієї системи; якщо програму не знайдено — системна гучність з підписом «Система».
- ⏩ **Перемотка** — клік або перетягування по прогрес-бару (лише якщо плеєр це дозволяє).
- 🎤 **Синхронізований текст пісні** окремим острівцем поруч з основним: щойно проспіваний, поточний і наступний рядок; на вступі — ♪. Джерело — [LRCLIB](https://lrclib.net), знайдені тексти кешуються. Кнопка 🎤 у плеєрі ховає / показує текст.
- ↔️ **Позиція острова** — вгорі або внизу екрана (над панеллю задач); острів можна перетягнути мишею, він «прилипає» до найближчого краю, а біля бокового краю розкривається всередину екрана. У налаштуваннях — «Вгорі / Внизу» і «Скинути позицію».

### Виправлено
- Текст пісні для браузерних плеєрів (SoundCloud, YouTube): назва на кшталт «Виконавець - Пісня (Official Video)» розбирається правильно.
- Прогрес-бар і текст пісні більше не зникають, коли Firefox на мить повідомляє тривалість 0.

### Відомі обмеження
- Показується лише синхронізований текст; якщо в LRCLIB є тільки звичайний — острівця з текстом немає.
- Острів працює на основному моніторі.

---

### New (EN)
- 🔊 Per-app volume (slider and mouse wheel), falls back to system volume.
- ⏩ Seeking by clicking or dragging the progress bar.
- 🎤 Synced lyrics from LRCLIB in a separate small island (previous / current / next line).
- ↔️ Island position: top or bottom, drag with snapping, edge anchoring, reset in Settings.

### Fixed (EN)
- Lyrics for browser players (SoundCloud, YouTube); progress and lyrics no longer disappear on Firefox's momentary 0 duration.

## v0.1.0 — 12.11.2026

Перша версія (MVP): острів з годинником, плеєр (GSMTC), статус і дозволи Claude Code через HTTP-hooks, UA / EN, налаштування, автозапуск, інсталятор.

First release (MVP): clock island, media player (GSMTC), Claude Code status and permissions via HTTP hooks, UA / EN, settings, autostart, installer.
