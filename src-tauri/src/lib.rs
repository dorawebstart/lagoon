// lib.rs — «серце» застосунку. Тут ми збираємо Tauri: підключаємо модулі,
// реєструємо команди і плагіни.

// `mod назва;` каже компілятору: «є модуль, його код — у файлі назва.rs».
// Без цих рядків файли просто ігнорувалися б.
mod agent;
mod audio;
mod media;
mod settings;
mod tray;
mod window;

// `use` імпортує трейти. Трейт — це набір методів, які тип «вміє»
// (схоже на interface у PHP). Manager додає застосунку get_webview_window,
// Listener — метод listen. Без імпорту ці методи «не видно».
use tauri::{Listener, Manager};

/// Точка запуску застосунку. `pub` означає, що функцію видно поза цим файлом:
/// її викликає main.rs.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Автозапуск разом з Windows (запис у реєстрі: HKCU, розділ Run)
        .plugin(tauri_plugin_autostart::Builder::new().build())
        // Плагін, що зберігає налаштування у JSON-файлі
        .plugin(tauri_plugin_store::Builder::new().build())
        // manage — реєструє спільний стан, доступний з команд і потоків
        .manage(window::HitRegion::default())
        .manage(media::MediaStore::default())
        // Команди, які можна викликати з JS через invoke
        .invoke_handler(tauri::generate_handler![
            window::set_hit_region,
            media::media_current,
            media::media_control,
            audio::audio_volume,
            audio::audio_set_volume,
            agent::agent_info,
            agent::agent_respond
        ])
        // setup виконується один раз після старту, коли вікна вже створено.
        // `|app| { ... }` — замикання (closure), аналог стрілкової функції в JS.
        .setup(|app| {
            // handle() — «ручка» застосунку, яку можна копіювати і передавати.
            let handle = app.handle().clone();

            // Трей мовою, яку користувач обрав раніше
            tray::create(&handle, &settings::saved_language(&handle))?;

            // Фоновий потік, що стежить за музикою (Spotify, YouTube…)
            media::start(handle.clone());

            // HTTP-сервер для hooks Claude Code (127.0.0.1:47823).
            // manage() тут, а не в Builder, бо серверу потрібен уже готовий
            // застосунок (AppHandle) і сховище налаштувань з токеном.
            app.manage(agent::start(&handle));

            // Коли в налаштуваннях змінили мову — перекладаємо меню трею.
            // `move` переносить копію `handle` всередину замикання,
            // бо воно житиме довше, ніж функція setup.
            let listener_handle = handle.clone();
            app.listen(settings::CHANGED_EVENT, move |event| {
                // payload() — JSON-рядок; serde_json перетворює його на нашу структуру.
                // `if let A && let B` — обидві умови мають «збігтися»: JSON розібрався (Ok)
                // і в ньому є мова (Some). Інакше просто нічого не робимо.
                if let Ok(changed) =
                    serde_json::from_str::<settings::SettingsChanged>(event.payload())
                    && let Some(language) = changed.language
                {
                    let _ = tray::set_language(&listener_handle, &language);
                }
            });

            // Шукаємо вікно за label з tauri.conf.json. Повертає Option,
            // бо вікна з такою назвою може й не бути.
            if let Some(island) = app.get_webview_window("main") {
                window::place_top_center(&island)?;
                // Вікно створене невидимим (visible: false), щоб не «стрибало»:
                // спершу ставимо на місце, потім показуємо.
                island.show()?;
                // Тут `island` передаємо без `&`, тобто переміщуємо (move):
                // після цього рядка змінною `island` користуватися вже не можна.
                window::start_cursor_watcher(island);
            }
            Ok(())
        })
        // generate_context! читає tauri.conf.json під час компіляції
        .run(tauri::generate_context!())
        // run повертає Result; expect зупинить програму з повідомленням, якщо сталася помилка
        .expect("не вдалося запустити застосунок Tauri");
}
