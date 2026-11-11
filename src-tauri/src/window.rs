// window.rs — усе, що стосується вікна-острова: позиція на екрані,
// click-through, а згодом і зміна розміру.

use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use tauri::{Manager, PhysicalPosition, State, WebviewWindow};

/// Як часто перевіряти, де курсор. 30 мс ≈ 33 рази на секунду: досить швидко,
/// щоб не помічати затримки, і майже не навантажує процесор.
const POLL_INTERVAL: Duration = Duration::from_millis(30);

/// Прямокутник у CSS-пікселях відносно лівого верхнього кута вікна —
/// саме в таких одиницях його віддає `getBoundingClientRect()` у JS.
///
/// - `#[derive(...)]` — макрос, який автоматично «дописує» типу вміння:
///   `Clone`/`Copy` — копіювати значення, `Default` — створити нульовий
///   прямокутник, `Deserialize` — зібрати його з JSON від фронтенду.
#[derive(Clone, Copy, Default, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    /// Чи лежить точка (у CSS-пікселях) всередині прямокутника.
    /// `&self` — метод лише читає прямокутник, нічого в ньому не змінює.
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }
}

/// Спільний стан: де зараз пігулка. Його пише команда з JS (потік інтерфейсу),
/// а читає фоновий потік click-through.
///
/// `Mutex` (mutual exclusion) — «замок»: дає доступ до даних лише одному
/// потоку за раз, тому вони не зіпсуються, коли два потоки працюють одночасно.
#[derive(Default)]
pub struct HitRegion(Mutex<Rect>);

/// Команда для JS: `invoke("set_hit_region", { rect: { x, y, width, height } })`.
///
/// `State<HitRegion>` Tauri підставляє сам: це доступ до стану, який ми
/// зареєстрували через `.manage(...)` у lib.rs.
#[tauri::command]
pub fn set_hit_region(rect: Rect, region: State<HitRegion>) {
    // lock() бере замок і дає доступ до Rect; unwrap() — якщо інший потік
    // «впав», тримаючи замок, програма теж зупиниться (у нас такого не буває).
    // `*... = rect` — записуємо нове значення туди, куди вказує замок.
    *region.0.lock().unwrap() = rect;
    // Замок звільняється автоматично в кінці рядка — нічого «відмикати» не треба.
}

/// Запускає фоновий потік, який вмикає click-through, коли курсор поза
/// пігулкою, і вимикає його, коли курсор над пігулкою.
///
/// Чому так: Windows вміє робити вікно або повністю «прозорим для миші»,
/// або повністю звичайним. Тож ми самі стежимо за курсором і перемикаємо режим.
///
/// `window: WebviewWindow` (без `&`) — функція забирає вікно у власність,
/// бо далі передає його в потік, який житиме довше за саму функцію.
pub fn start_click_through(window: WebviewWindow) {
    // `move` — замикання забирає змінну `window` собі (переміщує власність у потік).
    thread::spawn(move || {
        // Останній встановлений режим. None = ще нічого не встановлювали.
        let mut ignoring: Option<bool> = None;

        loop {
            // Якщо щось пішло не так (наприклад, вікно закривається) — вважаємо,
            // що курсор поза пігулкою.
            let over_island = cursor_over_island(&window).unwrap_or(false);
            let should_ignore = !over_island;

            // Кличемо Windows API лише тоді, коли режим справді змінився.
            if ignoring != Some(should_ignore)
                && window.set_ignore_cursor_events(should_ignore).is_ok()
            {
                ignoring = Some(should_ignore);
            }

            thread::sleep(POLL_INTERVAL);
        }
    });
}

/// Чи курсор зараз над пігулкою.
fn cursor_over_island(window: &WebviewWindow) -> tauri::Result<bool> {
    // Координати курсора і вікна — у фізичних пікселях екрана.
    let cursor = window.cursor_position()?;
    let origin = window.inner_position()?;
    // Масштаб Windows (100% = 1.0, 125% = 1.25...). CSS-пікселі = фізичні / масштаб.
    let scale = window.scale_factor()?;

    let x = (cursor.x - origin.x as f64) / scale;
    let y = (cursor.y - origin.y as f64) / scale;

    // state::<HitRegion>() дістає спільний стан. Завдяки `Copy` ми одразу
    // копіюємо Rect і відпускаємо замок, не тримаючи його довше, ніж треба.
    let region = *window.state::<HitRegion>().0.lock().unwrap();
    Ok(region.contains(x, y))
}

/// Ставить вікно по центру верхнього краю основного монітора.
///
/// - `window: &WebviewWindow` — позичаємо вікно (borrowing): функція лише
///   керує ним, але не забирає у власність, тож після виклику вікно
///   залишається доступним тому, хто його передав.
/// - `-> tauri::Result<()>` — функція або успішна (`Ok(())`, де `()` означає
///   «нічого не повертаємо»), або повертає помилку (`Err(...)`).
pub fn place_top_center(window: &WebviewWindow) -> tauri::Result<()> {
    // `?` після виклику: якщо там помилка — одразу повертаємо її з нашої
    // функції; якщо успіх — дістаємо значення і йдемо далі.
    //
    // primary_monitor() повертає Option<Monitor>: монітор може бути (Some)
    // або ні (None) — наприклад, у дуже рідкісних випадках без дисплея.
    let monitor = match window.primary_monitor()? {
        Some(monitor) => monitor,
        None => return Ok(()), // немає монітора — просто нічого не робимо
    };

    // Робоча область = екран мінус панель задач. Якщо панель задач
    // угорі, острів стане під нею, а не поверх неї.
    let area = monitor.work_area();
    let window_size = window.outer_size()?;

    // Розміри — беззнакові u32, координати — знакові i32 (на другому
    // моніторі ліворуч x може бути від'ємним). `as i32` — явне перетворення
    // типу: Rust не робить таких перетворень сам.
    let free_space = area.size.width as i32 - window_size.width as i32;
    let x = area.position.x + free_space / 2;
    let y = area.position.y;

    window.set_position(PhysicalPosition::new(x, y))?;
    Ok(())
}
