// visibility.rs — сховати або показати острів.
//
// Коли острів заважає (гра чи фільм на весь екран) або, навпаки, потрібен
// поверх такої програми, його можна сховати й повернути:
//   - гаряча клавіша Ctrl+Alt+L — працює з будь-якої програми;
//   - пункт «Сховати острів / Показати острів» у меню трею;
//   - запит дозволу від Claude Code показує прихований острів сам.
//
// Обмеження: поверх гри в ексклюзивному повноекранному режимі Windows не
// показує жодних вікон — там острів не з'явиться. У режимі «вікно без рамки»
// (borderless) працює.
//
// Стан не зберігаємо: після перезапуску острів знову видно — щоб не загубити його.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager};

use crate::{settings, tray};

/// Гаряча клавіша (формат плагіна global-shortcut).
pub const SHORTCUT: &str = "ctrl+alt+l";

/// Чи видно острів. Читають кілька потоків (курсор, рівень звуку), тому `AtomicBool`.
pub struct IslandVisible(AtomicBool);

/// Default вручну: на старті острів видно (true), а не false, як дав би derive.
impl Default for IslandVisible {
    fn default() -> Self {
        IslandVisible(AtomicBool::new(true))
    }
}

/// Чи видно острів зараз.
pub fn is_visible(app: &AppHandle) -> bool {
    app.state::<IslandVisible>().0.load(Ordering::Relaxed)
}

/// Сховати, якщо видно, і навпаки (гаряча клавіша, меню трею).
pub fn toggle(app: &AppHandle) {
    set(app, !is_visible(app));
}

/// Показати, якщо острів схований (наприклад, прийшов запит дозволу).
pub fn show(app: &AppHandle) {
    if !is_visible(app) {
        set(app, true);
    }
}

fn set(app: &AppHandle, visible: bool) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let result = if visible {
        window.show()
    } else {
        window.hide()
    };
    if let Err(error) = result {
        eprintln!("visibility: {error}");
        return;
    }
    app.state::<IslandVisible>()
        .0
        .store(visible, Ordering::Relaxed);
    // Пункт меню трею міняє текст: «Сховати острів» ↔ «Показати острів»
    let _ = tray::set_language(app, &settings::saved_language(app));
}
