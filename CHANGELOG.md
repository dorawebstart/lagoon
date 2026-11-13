# Що нового / Changelog

## v0.3.1 — 13.11.2026

### Виправлено
- Повторний запуск (ярлик, автозапуск) більше не відкриває ще один острів — показує вже запущений.
- Автозапуск із Windows знову працює, якщо в реєстрі лишився старий шлях до програми: на старті шлях оновлюється.

### Fixed (EN)
- Launching Lagoon again no longer opens a second island — it shows the running one.
- Autostart works again when the registry still points to an old path: the path is refreshed on start.

## v0.3.0 «Стиль» — 12.11.2026

### Нове
- 🎨 **Теми** — Класична, Графіт, Скло (напівпрозора з рамкою) і свій колір.
- 🌈 **Кольори** — акцент з обкладинки пісні, свій або стандартний (еквалайзер, прогрес, гучність, поточний рядок тексту); свій колір тексту; готові палітри «Океан», «Захід», «Ліс», «Лаванда».
- 📏 **Розміри** — масштаб острова 90–130 %, ширина й шрифт тексту пісні; вікно підлаштовується саме.
- 🔤 **Шрифт пісні** — Стандартний, Bahnschrift або Georgia для назви, виконавця й тексту пісні.
- 🌊 **Хвиля в такт музиці** вздовж краю острова — від справжнього рівня звуку програми, що грає; у розкритому плеєрі — висока.
- 💧 **Анімації** — плавне розкриття з «желе», крапля на нову пісню і «Готово ✓» від Claude Code, острівець з текстом виїжджає з-за острова; затримка 150 мс, щоб острів не розкривався від випадкового руху миші. «Зменшити рух» у Windows вимикає ефекти.
- 🙈 **Сховати / показати острів** — `Ctrl+Alt+L` або меню трею (наприклад, під час гри на весь екран у режимі «вікно без рамки»); запит дозволу від Claude Code показує острів сам.

### Відомі обмеження
- Хвиля рухається від гучності, а не від частот.
- Поверх гри в ексклюзивному повноекранному режимі Windows не показує острів.
- «Скло» — імітація, без розмиття того, що під островом.

---

### New (EN)
- 🎨 Themes: Classic, Graphite, Glass, custom color.
- 🌈 Accent from the album cover / custom / default; custom text color; ready-made palettes.
- 📏 Island scale, lyrics width and font size; 🔤 song font (Default, Bahnschrift, Georgia).
- 🌊 Audio-reactive wave along the island edge.
- 💧 Smooth jelly expand, a drop on new song and Claude's "Done", hover delay; respects "reduce motion".
- 🙈 Hide / show the island: `Ctrl+Alt+L` or the tray menu.

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
