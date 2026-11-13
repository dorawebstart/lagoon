// lib.rs — «серце» застосунку. Тут ми збираємо Tauri: підключаємо модулі,
// реєструємо команди і плагіни.

// `mod назва;` каже компілятору: «є модуль, його код — у файлі назва.rs».
// Без цих рядків файли просто ігнорувалися б.
mod agent;
mod audio;
mod lyrics;
mod media;
mod meter;
mod position;
mod settings;
mod tray;
mod visibility;
mod weather;
mod window;

// `use` імпортує трейти. Трейт — це набір методів, які тип «вміє»
// (схоже на interface у PHP). Manager додає застосунку get_webview_window,
// Listener — метод listen. Без імпорту ці методи «не видно».
use tauri::{Listener, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// Точка запуску застосунку. `pub` означає, що функцію видно поза цим файлом:
/// її викликає main.rs.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Лише одна копія Lagoon. Плагін має йти ПЕРШИМ: коли програму
        // запускають вдруге (ярлик, автозапуск), нова копія одразу
        // завершується, а в уже запущеній викликається це замикання.
        // Аргументи й робоча папка нам не потрібні — тому `_`.
        // Острів могли сховати (Ctrl+Alt+L) — тоді показуємо його.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            visibility::show(app);
        }))
        // Автозапуск разом з Windows (запис у реєстрі: HKCU, розділ Run)
        .plugin(tauri_plugin_autostart::Builder::new().build())
        // Плагін, що зберігає налаштування у JSON-файлі
        .plugin(tauri_plugin_store::Builder::new().build())
        // manage — реєструє спільний стан, доступний з команд і потоків
        .manage(window::HitRegion::default())
        .manage(media::MediaStore::default())
        .manage(position::DragState::default())
        .manage(position::LayoutState::default())
        .manage(meter::WaveEnabled::default())
        .manage(visibility::IslandVisible::default())
        .manage(weather::WeatherState::default())
        // Команди, які можна викликати з JS через invoke
        .invoke_handler(tauri::generate_handler![
            window::set_hit_region,
            media::media_current,
            media::media_control,
            media::media_seek,
            audio::audio_volume,
            audio::audio_set_volume,
            meter::audio_wave,
            lyrics::lyrics_get,
            position::island_placement,
            position::island_drag_start,
            weather::weather_current,
            weather::weather_search_city,
            weather::weather_locate,
            agent::agent_info,
            agent::agent_respond
        ])
        // setup виконується один раз після старту, коли вікна вже створено.
        // `|app| { ... }` — замикання (closure), аналог стрілкової функції в JS.
        .setup(|app| {
            // handle() — «ручка» застосунку, яку можна копіювати і передавати.
            let handle = app.handle().clone();

            // Автозапуск увімкнений, але запис у реєстрі міг лишитися зі старим
            // шляхом (інша папка встановлення, стара збірка) — тоді Windows
            // нічого не запускає. Перезаписуємо шлях на поточний lagoon.exe.
            refresh_autostart(&handle);

            // Гаряча клавіша Ctrl+Alt+L — сховати / показати острів.
            // with_handler: замикання викликається на кожне натискання і
            // відпускання; реагуємо лише на натискання.
            handle.plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(|app, _shortcut, event| {
                        if event.state == ShortcutState::Pressed {
                            visibility::toggle(app);
                        }
                    })
                    .build(),
            )?;
            // Комбінацію могла вже зайняти інша програма — тоді просто пишемо
            // в лог: острів однаково можна сховати з меню трею.
            if let Err(error) = handle.global_shortcut().register(visibility::SHORTCUT) {
                eprintln!("гаряча клавіша {}: {error}", visibility::SHORTCUT);
            }

            // Трей мовою, яку користувач обрав раніше
            tray::create(&handle, &settings::saved_language(&handle))?;

            // Фоновий потік, що стежить за музикою (Spotify, YouTube…)
            media::start(handle.clone());

            // Рівень звуку для «хвилі» (працює лише поки вона увімкнена і грає музика)
            meter::start(handle.clone());

            // Погода (Open-Meteo): раз на 30 хв, лише коли острів видно і місто обране
            weather::start(handle.clone());

            // HTTP-сервер для hooks Claude Code (127.0.0.1:47823).
            // manage() тут, а не в Builder, бо серверу потрібен уже готовий
            // застосунок (AppHandle) і сховище налаштувань з токеном.
            app.manage(agent::start(&handle));

            // Коли в налаштуваннях змінили мову — перекладаємо меню трею,
            // а позицію острова (верх/низ, «Скинути позицію») — переставляємо вікно.
            // `move` переносить копію `handle` всередину замикання,
            // бо воно житиме довше, ніж функція setup.
            let listener_handle = handle.clone();
            app.listen(settings::CHANGED_EVENT, move |event| {
                // Могли змінити місто чи вимкнути погоду — хай перевірить одразу
                // (сама вирішить, чи треба питати сервер)
                weather::settings_changed(&listener_handle);
                // payload() — JSON-рядок; serde_json перетворює його на нашу структуру.
                // `if let A && let B` — обидві умови мають «збігтися»: JSON розібрався (Ok)
                // і в ньому є мова (Some). Інакше просто нічого не робимо.
                let Ok(changed) =
                    serde_json::from_str::<settings::SettingsChanged>(event.payload())
                else {
                    return;
                };
                if let Some(language) = changed.language {
                    let _ = tray::set_language(&listener_handle, &language);
                }
                let Some(island) = listener_handle.get_webview_window("main") else {
                    return;
                };
                // Позиція з події (або збережена, якщо в події її немає).
                // Старі налаштування без islandAlign — вважаємо, що по центру.
                let target = match (changed.island_edge, changed.island_offset) {
                    (Some(edge), Some(offset)) => position::IslandPosition {
                        edge,
                        align: changed.island_align.unwrap_or_default(),
                        offset,
                    },
                    _ => position::saved(&listener_handle),
                };
                // Змінилися розміри — міняємо розмір вікна; інакше лише позицію.
                let layout = position::layout_from(changed.island_scale, changed.lyrics_width);
                if position::layout_changed(&island, layout) {
                    let _ = position::apply_layout(&island, layout, target);
                } else {
                    let _ = position::place(&island, target);
                }
            });

            // Шукаємо вікно за label з tauri.conf.json. Повертає Option,
            // бо вікна з такою назвою може й не бути.
            if let Some(island) = app.get_webview_window("main") {
                // Збережена позиція (вгорі/внизу, зсув) або верх по центру.
                position::apply_layout(
                    &island,
                    position::saved_layout(&handle),
                    position::saved(&handle),
                )?;
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

/// Якщо автозапуск увімкнений — записує в реєстр шлях до поточного lagoon.exe.
///
/// `#[cfg(not(debug_assertions))]` — функція компілюється лише в release-збірці
/// (інсталятор). У dev-збірці (`npm run tauri dev`) запис не чіпаємо, інакше
/// автозапуск почав би вказувати на тимчасову dev-версію в папці проєкту.
#[cfg(not(debug_assertions))]
fn refresh_autostart(app: &tauri::AppHandle) {
    // ManagerExt — трейт плагіна: додає застосунку метод autolaunch()
    use tauri_plugin_autostart::ManagerExt;

    let autolaunch = app.autolaunch();
    // is_enabled лише перевіряє, що запис є; enable перезаписує його шлях.
    // `matches!(значення, Ok(true))` — true, лише якщо вдалося і запис є.
    // `&& let Err(..)` — друга умова: перезаписати не вдалося → пишемо в лог.
    if matches!(autolaunch.is_enabled(), Ok(true))
        && let Err(error) = autolaunch.enable()
    {
        eprintln!("автозапуск: {error}");
    }
}

/// У dev-збірці — нічого не робимо (див. release-варіант вище).
#[cfg(debug_assertions)]
fn refresh_autostart(_app: &tauri::AppHandle) {}
