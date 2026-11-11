// lib.rs — «серце» застосунку. Тут ми збираємо Tauri: підключаємо модулі,
// реєструємо команди і плагіни.

// `mod window;` каже компілятору: «є модуль window, його код — у файлі window.rs».
// Без цього рядка файл window.rs просто ігнорувався б.
mod window;

// `use` імпортує трейт Manager. Трейт — це набір методів, які тип «вміє»
// (схоже на interface у PHP). Саме Manager додає застосунку метод
// get_webview_window, тому без цього імпорту виклик нижче не скомпілюється.
use tauri::Manager;

/// Точка запуску застосунку. `pub` означає, що функцію видно поза цим файлом:
/// її викликає main.rs.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // setup виконується один раз після старту, коли вікна вже створено.
        // `|app| { ... }` — замикання (closure), аналог стрілкової функції в JS.
        .setup(|app| {
            // Шукаємо вікно за label з tauri.conf.json. Повертає Option,
            // бо вікна з такою назвою може й не бути.
            if let Some(island) = app.get_webview_window("main") {
                window::place_top_center(&island)?;
                // Вікно створене невидимим (visible: false), щоб не «стрибало»:
                // спершу ставимо на місце, потім показуємо.
                island.show()?;
            }
            Ok(())
        })
        // generate_context! читає tauri.conf.json під час компіляції
        .run(tauri::generate_context!())
        // run повертає Result; expect зупинить програму з повідомленням, якщо сталася помилка
        .expect("не вдалося запустити застосунок Tauri");
}
