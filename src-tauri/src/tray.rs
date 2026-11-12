// tray.rs — іконка в треї (біля годинника Windows) з меню:
// «Налаштування», «Перезапустити», «Вийти».

use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};

use crate::window;

/// Ідентифікатор іконки — за ним потім знаходимо її, щоб змінити меню.
const TRAY_ID: &str = "lagoon-tray";

/// Тексти меню однією мовою.
///
/// `&'static str` — посилання на рядок, який живе весь час роботи програми
/// (він «вшитий» у .exe). `'static` — це lifetime, «час життя» посилання.
struct Labels {
    settings: &'static str,
    restart: &'static str,
    quit: &'static str,
}

/// Тексти меню для мови. Решта UI перекладається в JS (src/locales),
/// а меню трею малює Windows, тому ці три рядки живуть тут.
fn labels(language: &str) -> Labels {
    // match по рядку: як switch у JS. `_` — «усе інше».
    match language {
        "en" => Labels {
            settings: "Settings",
            restart: "Restart",
            quit: "Quit",
        },
        _ => Labels {
            settings: "Налаштування",
            restart: "Перезапустити",
            quit: "Вийти",
        },
    }
}

/// Створює меню трею потрібною мовою.
///
/// `Menu<Wry>` — тип узагальнений (generic): `Wry` означає «меню для
/// стандартного рушія вікон Tauri». Так у Rust пишуть типи-«шаблони».
fn build_menu(app: &AppHandle, language: &str) -> tauri::Result<Menu<Wry>> {
    let text = labels(language);

    // Перший аргумент — id пункту: за ним розпізнаємо, що натиснули.
    // true — пункт активний; None — без гарячої клавіші.
    let settings = MenuItem::with_id(app, "settings", text.settings, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let restart = MenuItem::with_id(app, "restart", text.restart, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", text.quit, true, None::<&str>)?;

    // &[...] — позичений масив посилань на пункти меню.
    Menu::with_items(app, &[&settings, &separator, &restart, &quit])
}

/// Створює іконку в треї. Викликається один раз під час старту.
pub fn create(app: &AppHandle, language: &str) -> tauri::Result<()> {
    let menu = build_menu(app, language)?;

    // `mut` — змінну можна змінювати. Без `mut` усі змінні в Rust незмінні.
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Lagoon")
        .menu(&menu)
        // Лівий клік відкриває налаштування, меню — лише правий клік.
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(on_tray_icon_event);

    // Беремо іконку застосунку (з src-tauri/icons). Її може не бути — тоді Option = None.
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    Ok(())
}

/// Перекладає меню трею (коли користувач змінив мову в налаштуваннях).
pub fn set_language(app: &AppHandle, language: &str) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(build_menu(app, language)?))?;
    }
    Ok(())
}

/// Обробник кліку по пункту меню.
fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    // event.id() — той самий id, що ми дали в MenuItem::with_id.
    match event.id().as_ref() {
        "settings" => window::open_settings(app),
        "restart" => app.restart(),
        "quit" => app.exit(0),
        _ => {}
    }
}

/// Обробник кліків по самій іконці.
fn on_tray_icon_event(tray: &TrayIcon, event: TrayIconEvent) {
    // Шаблон (pattern) з `..` — «решта полів не важлива».
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        window::open_settings(tray.app_handle());
    }
}
