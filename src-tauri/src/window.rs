// window.rs — вікно-острів: click-through, наведення і вікно налаштувань.
// Позиція острова на екрані та перетягування — у position.rs.

use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::position;

/// Label вікна налаштувань (його згадує capabilities/default.json).
pub const SETTINGS_LABEL: &str = "settings";

/// Аргументи для WebView2 (рушій сторінок у Windows). Мають бути однакові для
/// всіх вікон, бо вікна ділять один процес браузера.
///
/// `--disable-gpu` — малювати без відеокарти: для маленької пігулки з CSS-анімаціями
/// цього досить, а WebView2 не запускає окремий GPU-процес (~80 МБ пам'яті).
/// Решта — стандартні аргументи Tauri, які треба зберегти, коли задаємо свої.
/// Те саме значення стоїть в `additionalBrowserArgs` вікна острова в tauri.conf.json.
pub const BROWSER_ARGS: &str =
    "--disable-gpu --disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

/// Відкриває вікно налаштувань (або показує вже відкрите).
///
/// Вікно створюється лише тоді, коли воно потрібне, і повністю закривається
/// хрестиком: кожне вікно WebView2 — це окремий процес, тож поки налаштування
/// закриті, острів займає менше пам'яті.
pub fn open_settings(app: &AppHandle) {
    if let Some(settings) = app.get_webview_window(SETTINGS_LABEL) {
        // Помилки ігноруємо: якщо не вдалося, острів однаково працює далі.
        let _ = settings.unminimize();
        let _ = settings.set_focus();
        return;
    }

    // Builder: ланцюжок налаштувань, а в кінці build() створює вікно.
    let result =
        WebviewWindowBuilder::new(app, SETTINGS_LABEL, WebviewUrl::App("settings.html".into()))
            .title("Lagoon")
            .inner_size(440.0, 760.0)
            .resizable(false)
            .maximizable(false)
            .center()
            .additional_browser_args(BROWSER_ARGS)
            .build();

    if let Err(error) = result {
        eprintln!("settings: не вдалося відкрити вікно: {error}");
    }
}

/// Назва події, яку слухає фронтенд: `listen("island:hover", ...)`.
pub const HOVER_EVENT: &str = "island:hover";

/// Як часто перевіряти, де курсор. 30 мс ≈ 33 рази на секунду: досить швидко,
/// щоб не помічати затримки, і майже не навантажує процесор.
const POLL_INTERVAL: Duration = Duration::from_millis(30);

/// Під час перетягування — частіше, щоб вікно плавно йшло за курсором.
const DRAG_INTERVAL: Duration = Duration::from_millis(10);

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

impl HitRegion {
    /// Поточний прямокутник пігулки (копія — замок одразу відпускається).
    pub fn get(&self) -> Rect {
        *self.0.lock().unwrap()
    }
}

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

/// Запускає фоновий потік, який стежить за курсором і:
/// 1. вмикає click-through, коли курсор поза островом, і вимикає над ним;
/// 2. надсилає фронтенду подію `island:hover` (true / false), щоб той
///    розкрив або згорнув острів.
///
/// Чому так: Windows вміє робити вікно або повністю «прозорим для миші»,
/// або повністю звичайним. Тож ми самі стежимо за курсором і перемикаємо режим.
/// А коли вікно «прозоре для миші», сторінка не отримує подій миші взагалі —
/// тому CSS `:hover` ненадійний, і про наведення фронтенду повідомляє Rust.
///
/// `window: WebviewWindow` (без `&`) — функція забирає вікно у власність,
/// бо далі передає його в потік, який житиме довше за саму функцію.
pub fn start_cursor_watcher(window: WebviewWindow) {
    // `move` — замикання забирає змінну `window` собі (переміщує власність у потік).
    thread::spawn(move || {
        // Останній відомий стан. None = ще нічого не встановлювали.
        let mut last_over: Option<bool> = None;

        loop {
            // Тягнемо острів мишею: рухом вікна займається position.rs, а
            // click-through і наведення не чіпаємо, щоб не перервати перетягування.
            if position::drag_tick(&window) {
                // Після перетягування стан наведення треба перевірити заново.
                last_over = None;
                thread::sleep(DRAG_INTERVAL);
                continue;
            }

            // Якщо щось пішло не так (наприклад, вікно закривається) — вважаємо,
            // що курсор поза островом.
            let over_island = cursor_over_island(&window).unwrap_or(false);

            // Робимо щось лише тоді, коли стан справді змінився.
            if last_over != Some(over_island)
                && window.set_ignore_cursor_events(!over_island).is_ok()
            {
                last_over = Some(over_island);
                // emit — надіслати подію у фронтенд. Результат нас не цікавить:
                // `let _ =` явно каже «ігнорую Result» (інакше буде попередження).
                let _ = window.emit(HOVER_EVENT, over_island);
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
