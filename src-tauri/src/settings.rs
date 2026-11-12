// settings.rs — налаштування, які зберігаються у файлі settings.json
// (його веде плагін tauri-plugin-store; JS теж читає і пише цей файл).

use serde::Deserialize;
use tauri::AppHandle;

use crate::position::{IslandAlign, IslandEdge};
use tauri_plugin_store::StoreExt;

/// Файл налаштувань. Лежить у теці даних застосунку:
/// %APPDATA%\com.dorawebstart.lagoon\settings.json
pub const STORE_FILE: &str = "settings.json";

/// Подія, яку вікно налаштувань надсилає після кожної зміни.
pub const CHANGED_EVENT: &str = "settings:changed";

/// Мова за замовчуванням (див. PLAN.md, 4.4).
pub const DEFAULT_LANGUAGE: &str = "uk";

/// Те, що приходить у події `settings:changed`. Нас у Rust цікавить мова
/// (щоб перекласти меню в треї) і позиція острова; решту полів serde пропустить.
///
/// `Option<...>` — поле може бути відсутнім у JSON, тоді буде `None`.
/// `rename_all = "camelCase"` — у JSON поля звуться `islandEdge`, `islandAlign`, `islandOffset`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsChanged {
    pub language: Option<String>,
    pub island_edge: Option<IslandEdge>,
    pub island_align: Option<IslandAlign>,
    pub island_offset: Option<f64>,
}

/// Мова, збережена в налаштуваннях, або "uk", якщо її ще не обирали.
///
/// Повертаємо `String` (а не `&str`), бо рядок створюється тут, всередині
/// функції, і має «жити» після її завершення — тож віддаємо його у власність.
pub fn saved_language(app: &AppHandle) -> String {
    // store(...) повертає Result: файл міг не відкритися.
    let store = match app.store(STORE_FILE) {
        Ok(store) => store,
        Err(_) => return DEFAULT_LANGUAGE.to_string(),
    };

    // get(...) повертає Option<serde_json::Value> — довільне JSON-значення.
    // as_str() перевіряє, що це саме рядок, і теж повертає Option.
    match store.get("language") {
        Some(value) => match value.as_str() {
            Some(language) => language.to_string(),
            None => DEFAULT_LANGUAGE.to_string(),
        },
        None => DEFAULT_LANGUAGE.to_string(),
    }
}
