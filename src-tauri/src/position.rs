// position.rs — де стоїть острів: вгорі чи внизу екрана і наскільки зсунутий
// вліво/вправо. Тут же — перетягування острова мишею і «прилипання» до краю.
//
// Позиція зберігається в settings.json двома полями:
//   islandEdge   — "top" | "bottom";
//   islandAlign  — "left" | "center" | "right": до чого прив'язаний острів;
//   islandOffset — де «точка прив'язки» острова: зсув від центру робочої
//                  області у частках її ширини (−0.5…0.5). Частки, а не
//                  пікселі, — щоб позиція не «з'їжджала» після зміни роздільної здатності.
//
// Точка прив'язки: посередині екрана — центр острова (розкривається в обидва
// боки); біля лівого краю — лівий край острова (розкривається вправо);
// біля правого — правий край (розкривається вліво). Так згорнута пігулка,
// кинута в куток, лишається в кутку, а розгорнута не вилазить за екран.
//
// Усе рахуємо від робочої області основного монітора (екран мінус панель
// задач), тож унизу острів стоїть над панеллю задач, а не під нею.
//
// Розміри (крок 18): масштаб острова і ширина острівця з текстом пісні
// (налаштування islandScale, lyricsWidth) змінюють і розмір вікна, і межі,
// до яких острів може підходити до краю екрана — див. `Layout`.
//
// Перетягування: фронтенд (drag.js) лише повідомляє «почали тягнути»
// (команда island_drag_start). Далі потік, який і так стежить за курсором
// (window.rs), кожні 10 мс зсуває вікно за курсором, а коли ліва кнопка миші
// відпущена — «прилипає» до найближчого краю, зберігає позицію і надсилає
// фронтенду подію island:drag-end.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, State, WebviewWindow};
use tauri_plugin_store::StoreExt;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};

use crate::settings::STORE_FILE;
use crate::window::HitRegion;

/// Подія для фронтенду: край, вирівнювання і з якого боку текст пісні.
pub const PLACEMENT_EVENT: &str = "island:placement";
/// Подія для фронтенду: перетягування завершилося.
pub const DRAG_END_EVENT: &str = "island:drag-end";

/// Половина розгорнутого острова (380 / 2) у CSS px при масштабі 100 %.
const ISLAND_HALF: f64 = 190.0;
/// Відступ (CSS px) острова від краю вікна й екрана, коли він прив'язаний до краю.
/// Має збігатися з margin у island.css (body.align-left / align-right).
const EDGE_GAP: f64 = 10.0;
/// Проміжок (CSS px) між основним островом і острівцем з текстом пісні.
const LYRICS_GAP: f64 = 10.0;
/// Мінімальний розмір вікна (CSS px) — як було до кроку 18.
const MIN_WINDOW_WIDTH: f64 = 1000.0;
/// 216, а не 200: запас для анімацій (желе при розкритті, крапля), щоб нічого не обрізалось.
const BASE_WINDOW_HEIGHT: f64 = 216.0;

/// Край екрана. `#[serde(rename_all = "lowercase")]` — у JSON це "top" / "bottom".
/// `#[default]` — значення для `IslandEdge::default()` (верх).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IslandEdge {
    #[default]
    Top,
    Bottom,
}

/// До чого прив'язаний острів по горизонталі (див. пояснення на початку файлу).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IslandAlign {
    Left,
    #[default]
    Center,
    Right,
}

/// З якого боку від острова показувати текст пісні.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
}

/// Збережена позиція острова.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IslandPosition {
    pub edge: IslandEdge,
    pub align: IslandAlign,
    /// Зсув точки прив'язки від центру робочої області, у частках її ширини.
    pub offset: f64,
}

/// Що отримує фронтенд у події island:placement і з команди island_placement.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    pub edge: IslandEdge,
    pub align: IslandAlign,
    pub lyrics_side: Side,
}

/// Розміри острівців з налаштувань.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    /// Масштаб основного острова: 1.0 = 100 %.
    pub island_scale: f64,
    /// Ширина острівця з текстом пісні в CSS px (до масштабу).
    pub lyrics_width: f64,
}

/// `impl Default` вручну, бо «за замовчуванням» тут не нулі, а 100 % і 280 px.
impl Default for Layout {
    fn default() -> Self {
        Layout {
            island_scale: 1.0,
            lyrics_width: 280.0,
        }
    }
}

impl Layout {
    /// Скільки місця (CSS px) від центру острова до краю екрана, щоб
    /// розгорнутий острів уміщався: половина острова × масштаб + 10 px.
    fn half_room(&self) -> f64 {
        ISLAND_HALF * self.island_scale + EDGE_GAP
    }

    /// Скільки місця (CSS px) займає острівець з текстом разом із проміжком
    /// (обидва масштабуються разом з островом) і відступом від краю екрана.
    fn lyrics_room(&self) -> f64 {
        (self.lyrics_width + LYRICS_GAP) * self.island_scale + EDGE_GAP
    }

    /// Розмір вікна (CSS px): острів по центру, текст пісні з кожного боку
    /// (він може перескочити праворуч), висота — пропорційно масштабу.
    /// `.ceil()` — округлення вгору, щоб нічого не обрізалося.
    pub fn window_size(&self) -> (f64, f64) {
        let width = (2.0 * (self.half_room() + self.lyrics_room())).max(MIN_WINDOW_WIDTH);
        let height = (BASE_WINDOW_HEIGHT * self.island_scale).ceil();
        (width.ceil(), height)
    }
}

/// Поточні розміри — спільний стан: пише обробник налаштувань, читають
/// розрахунки позиції (у тому числі з потоку перетягування).
#[derive(Default)]
pub struct LayoutState(Mutex<Layout>);

/// Прямокутник робочої області у фізичних пікселях екрана.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Area {
    fn right(&self) -> f64 {
        self.x + self.width
    }

    fn center_x(&self) -> f64 {
        self.x + self.width / 2.0
    }
}

// ───────── Чисті розрахунки (без Windows, тому їх легко тестувати) ─────────

/// Точка прив'язки у фізичних пікселях, обмежена межами екрана.
/// `scale` — масштаб Windows (1.25 при 125 %): CSS px × scale = фізичні px.
fn anchor_x(area: Area, scale: f64, layout: Layout, position: IslandPosition) -> f64 {
    let wanted = area.center_x() + position.offset * area.width;
    let (min, max) = match position.align {
        // Центр — не ближче половини розгорнутого острова до краю
        IslandAlign::Center => (
            area.x + layout.half_room() * scale,
            area.right() - layout.half_room() * scale,
        ),
        // Край острова — не ближче 10 px до краю екрана
        IslandAlign::Left | IslandAlign::Right => {
            (area.x + EDGE_GAP * scale, area.right() - EDGE_GAP * scale)
        }
    };
    if min > max {
        area.center_x() // екран вужчий за острів (майже неможливо)
    } else {
        wanted.clamp(min, max)
    }
}

/// Лівий верхній кут вікна для позиції `position`.
pub fn window_origin(
    area: Area,
    window_width: f64,
    window_height: f64,
    scale: f64,
    layout: Layout,
    position: IslandPosition,
) -> (f64, f64) {
    let anchor = anchor_x(area, scale, layout, position);
    // Де острів усередині вікна — так його ставить CSS (island.css):
    let x = match position.align {
        // по центру вікна
        IslandAlign::Center => anchor - window_width / 2.0,
        // лівий край острова — за 10 px від лівого краю вікна
        IslandAlign::Left => anchor - EDGE_GAP * scale,
        // правий край острова — за 10 px від правого краю вікна
        IslandAlign::Right => anchor + EDGE_GAP * scale - window_width,
    };
    let y = match position.edge {
        IslandEdge::Top => area.y,
        // Унизу вікно прив'язане нижнім краєм до низу робочої області.
        IslandEdge::Bottom => area.y + area.height - window_height,
    };
    (x, y)
}

/// «Прилипання» після перетягування.
/// - край — за тим, у якій половині екрана курсор;
/// - вирівнювання — за тим, чи вміститься біля краю розгорнутий острів по центру:
///   якщо ні — прив'язуємо до цього краю, і острів лишається там, де його кинули.
///
/// `island_left` / `island_right` — де зараз (згорнутий) острів, фізичні px.
pub fn snap(
    area: Area,
    scale: f64,
    layout: Layout,
    island_left: f64,
    island_right: f64,
    cursor_y: f64,
) -> IslandPosition {
    let edge = if cursor_y < area.y + area.height / 2.0 {
        IslandEdge::Top
    } else {
        IslandEdge::Bottom
    };
    let center = (island_left + island_right) / 2.0;
    let room = layout.half_room() * scale;
    let (align, anchor) = if center - area.x < room {
        (IslandAlign::Left, island_left)
    } else if area.right() - center < room {
        (IslandAlign::Right, island_right)
    } else {
        (IslandAlign::Center, center)
    };
    let position = IslandPosition {
        edge,
        align,
        offset: (anchor - area.center_x()) / area.width,
    };
    // Ще раз через anchor_x, щоб збережене значення вже було в межах екрана.
    // `..position` — «решту полів узяти з position» (як spread у JS).
    IslandPosition {
        offset: (anchor_x(area, scale, layout, position) - area.center_x()) / area.width,
        ..position
    }
}

/// Текст пісні — ліворуч від острова, а якщо там не вміщається (острів біля
/// лівого краю екрана) — праворуч.
pub fn lyrics_side(area: Area, scale: f64, layout: Layout, position: IslandPosition) -> Side {
    match position.align {
        IslandAlign::Left => Side::Right,
        IslandAlign::Right => Side::Left,
        IslandAlign::Center => {
            let center = anchor_x(area, scale, layout, position);
            if center - area.x < (layout.half_room() + layout.lyrics_room()) * scale {
                Side::Right
            } else {
                Side::Left
            }
        }
    }
}

// ───────── Робота з вікном і налаштуваннями ─────────

/// Робоча область основного монітора і масштаб, або None, якщо монітора немає.
fn primary_area(window: &WebviewWindow) -> Option<(Area, f64)> {
    let monitor = window.primary_monitor().ok()??;
    let work = monitor.work_area();
    let area = Area {
        x: work.position.x as f64,
        y: work.position.y as f64,
        width: work.size.width as f64,
        height: work.size.height as f64,
    };
    Some((area, monitor.scale_factor()))
}

/// Позиція з settings.json (або верх по центру, якщо її ще не зберігали).
pub fn saved(app: &AppHandle) -> IslandPosition {
    let Ok(store) = app.store(STORE_FILE) else {
        return IslandPosition::default();
    };
    // from_value перетворює JSON-значення ("bottom") на наш enum через serde.
    let edge = store
        .get("islandEdge")
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    let align = store
        .get("islandAlign")
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    let offset = store
        .get("islandOffset")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    IslandPosition {
        edge,
        align,
        offset,
    }
}

/// Розміри з settings.json: islandScale у відсотках (90–130), lyricsWidth у px.
pub fn saved_layout(app: &AppHandle) -> Layout {
    let Ok(store) = app.store(STORE_FILE) else {
        return Layout::default();
    };
    let number = |key: &str| store.get(key).and_then(|value| value.as_f64());
    layout_from(number("islandScale"), number("lyricsWidth"))
}

/// Розміри з налаштувань (відсотки й px) з обмеженням допустимими межами.
pub fn layout_from(scale_percent: Option<f64>, lyrics_width: Option<f64>) -> Layout {
    let default = Layout::default();
    Layout {
        island_scale: scale_percent.map_or(default.island_scale, |p| p.clamp(90.0, 130.0) / 100.0),
        lyrics_width: lyrics_width.map_or(default.lyrics_width, |w| w.clamp(220.0, 420.0)),
    }
}

/// Поточні розміри (копія — замок одразу відпускається).
fn current_layout(window: &WebviewWindow) -> Layout {
    *window.state::<LayoutState>().0.lock().unwrap()
}

/// Застосовує нові розміри: запам'ятовує їх, змінює розмір вікна і ставить
/// острів у позицію `position` (від розміру вікна залежать координати).
pub fn apply_layout(
    window: &WebviewWindow,
    layout: Layout,
    position: IslandPosition,
) -> tauri::Result<()> {
    *window.state::<LayoutState>().0.lock().unwrap() = layout;
    let (width, height) = layout.window_size();
    // LogicalSize — CSS px; Tauri сам помножить на масштаб Windows.
    window.set_size(LogicalSize::new(width, height))?;
    place(window, position)
}

/// Чи відрізняються розміри від поточних (щоб не змінювати вікно без потреби).
pub fn layout_changed(window: &WebviewWindow, layout: Layout) -> bool {
    current_layout(window) != layout
}

/// Записує позицію в settings.json (той самий файл, що й у JS).
fn save(app: &AppHandle, position: IslandPosition) {
    if let Ok(store) = app.store(STORE_FILE) {
        store.set("islandEdge", json!(position.edge));
        store.set("islandAlign", json!(position.align));
        store.set("islandOffset", json!(position.offset));
        if let Err(error) = store.save() {
            eprintln!("position: не вдалося зберегти: {error}");
        }
    }
}

/// Ставить вікно в позицію `position` і повідомляє фронтенд, де тепер острів.
pub fn place(window: &WebviewWindow, position: IslandPosition) -> tauri::Result<()> {
    let Some((area, scale)) = primary_area(window) else {
        return Ok(());
    };
    let layout = current_layout(window);
    let size = window.outer_size()?;
    let (x, y) = window_origin(
        area,
        size.width as f64,
        size.height as f64,
        scale,
        layout,
        position,
    );
    // `as i32` — координати вікна цілі; дробову частину просто відкидаємо.
    window.set_position(PhysicalPosition::new(x as i32, y as i32))?;
    let _ = window.emit(PLACEMENT_EVENT, placement(area, scale, layout, position));
    Ok(())
}

fn placement(area: Area, scale: f64, layout: Layout, position: IslandPosition) -> Placement {
    Placement {
        edge: position.edge,
        align: position.align,
        lyrics_side: lyrics_side(area, scale, layout, position),
    }
}

/// Команда для JS: `invoke("island_placement")` — де острів зараз.
/// Потрібна при завантаженні сторінки: подія могла прийти раніше, ніж сторінка підписалася.
///
/// Параметр `window: WebviewWindow` Tauri підставляє сам — це вікно, яке викликало команду.
#[tauri::command]
pub fn island_placement(app: AppHandle, window: WebviewWindow) -> Option<Placement> {
    let (area, scale) = primary_area(&window)?;
    Some(placement(area, scale, current_layout(&window), saved(&app)))
}

// ───────── Перетягування ─────────

/// Звідки почали тягнути: курсор і вікно у фізичних пікселях.
#[derive(Clone, Copy)]
struct Drag {
    cursor_x: f64,
    cursor_y: f64,
    window_x: i32,
    window_y: i32,
}

/// Спільний стан перетягування: Some — тягнемо зараз, None — ні.
/// Пише команда з JS, читає потік курсора, тому — `Mutex`.
#[derive(Default)]
pub struct DragState(Mutex<Option<Drag>>);

/// Команда для JS: `invoke("island_drag_start")` — курсор зсунувся більше
/// ніж на 5 px від точки натискання, тож це перетягування, а не клік.
#[tauri::command]
pub fn island_drag_start(window: WebviewWindow, drag: State<DragState>) -> Result<(), String> {
    let cursor = window.cursor_position().map_err(|e| e.to_string())?;
    let origin = window.outer_position().map_err(|e| e.to_string())?;
    *drag.0.lock().unwrap() = Some(Drag {
        cursor_x: cursor.x,
        cursor_y: cursor.y,
        window_x: origin.x,
        window_y: origin.y,
    });
    Ok(())
}

/// Чи натиснута зараз ліва кнопка миші (питаємо саму Windows, а не сторінку).
fn left_button_down() -> bool {
    // Старший біт результату (0x8000) = кнопка натиснута саме зараз.
    // `as u16` — дивимося на результат як на беззнакове число, щоб перевірити біт.
    let state = unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON.0)) };
    (state as u16) & 0x8000 != 0
}

/// Один «крок» перетягування; його викликає потік курсора з window.rs.
/// Повертає true, якщо зараз тягнемо (тоді потік не чіпає click-through і наведення).
pub fn drag_tick(window: &WebviewWindow) -> bool {
    let state = window.state::<DragState>();
    // Копіюємо Drag і одразу відпускаємо замок (Drag — Copy).
    let Some(drag) = *state.0.lock().unwrap() else {
        return false;
    };

    let Ok(cursor) = window.cursor_position() else {
        return true;
    };

    if left_button_down() {
        // Вікно їде за курсором: стартова позиція + наскільки зсунувся курсор.
        let x = drag.window_x + (cursor.x - drag.cursor_x) as i32;
        let y = drag.window_y + (cursor.y - drag.cursor_y) as i32;
        let _ = window.set_position(PhysicalPosition::new(x, y));
        return true;
    }

    // Кнопку відпущено — завершуємо.
    *state.0.lock().unwrap() = None;
    finish_drag(window, cursor.y);
    let _ = window.emit(DRAG_END_EVENT, ());
    false
}

/// «Прилипає» до найближчого краю і зберігає позицію.
fn finish_drag(window: &WebviewWindow, cursor_y: f64) {
    let Some((area, scale)) = primary_area(window) else {
        return;
    };
    let Ok(origin) = window.outer_position() else {
        return;
    };
    // Де зараз острів: прямокутник пігулки (CSS px відносно вікна), який
    // фронтенд надсилає для click-through, переводимо у фізичні px екрана.
    let rect = window.state::<HitRegion>().get();
    let island_left = origin.x as f64 + rect.x * scale;
    let island_right = island_left + rect.width * scale;
    let layout = current_layout(window);
    let position = snap(area, scale, layout, island_left, island_right, cursor_y);
    save(window.app_handle(), position);
    let _ = place(window, position);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Монітор 1920×1080 з панеллю задач 40 px унизу, масштаб 100 %.
    const AREA: Area = Area {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1040.0,
    };

    /// Розміри за замовчуванням: 100 %, текст пісні 280 px.
    const L: Layout = Layout {
        island_scale: 1.0,
        lyrics_width: 280.0,
    };

    fn position(edge: IslandEdge, align: IslandAlign, offset: f64) -> IslandPosition {
        IslandPosition {
            edge,
            align,
            offset,
        }
    }

    #[test]
    fn places_top_center_by_default() {
        let (x, y) = window_origin(AREA, 1000.0, 200.0, 1.0, L, IslandPosition::default());
        assert_eq!((x, y), (460.0, 0.0));
    }

    #[test]
    fn places_bottom_above_taskbar() {
        let bottom = position(IslandEdge::Bottom, IslandAlign::Center, 0.0);
        let (_, y) = window_origin(AREA, 1000.0, 200.0, 1.0, L, bottom);
        assert_eq!(y, 840.0); // 1040 − 200: нижній край вікна на межі панелі задач
    }

    #[test]
    fn clamps_center_to_screen() {
        let far_left = position(IslandEdge::Top, IslandAlign::Center, -0.5);
        let (x, _) = window_origin(AREA, 1000.0, 200.0, 1.0, L, far_left);
        // Центр не ближче 200 px до краю → вікно 1000 px починається з −300.
        assert_eq!(x, -300.0);
    }

    #[test]
    fn island_dropped_in_corner_stays_there() {
        // Згорнутий острів 200 px кинули в лівий верхній кут: x від 10 до 210.
        let left = snap(AREA, 1.0, L, 10.0, 210.0, 20.0);
        assert_eq!(left.align, IslandAlign::Left);
        assert_eq!(left.edge, IslandEdge::Top);
        // Лівий край острова за 10 px від краю вікна → вікно починається з 0.
        let (x, _) = window_origin(AREA, 1000.0, 200.0, 1.0, L, left);
        assert_eq!(x, 0.0);

        // Правий нижній кут: x від 1710 до 1910.
        let right = snap(AREA, 1.0, L, 1710.0, 1910.0, 1000.0);
        assert_eq!(right.align, IslandAlign::Right);
        assert_eq!(right.edge, IslandEdge::Bottom);
        let (x, _) = window_origin(AREA, 1000.0, 200.0, 1.0, L, right);
        assert_eq!(x, 920.0); // 1910 + 10 − 1000
    }

    #[test]
    fn snaps_center_away_from_edges() {
        let middle = snap(AREA, 1.0, L, 1340.0, 1540.0, 900.0);
        assert_eq!(middle.align, IslandAlign::Center);
        assert_eq!(middle.edge, IslandEdge::Bottom);
        assert_eq!(middle.offset, 0.25); // центр 1440: (1440 − 960) / 1920
    }

    #[test]
    fn island_is_kept_inside_screen_when_dropped_past_edge() {
        // Кинули так, що острів наполовину за лівим краєм екрана.
        let left = snap(AREA, 1.0, L, -100.0, 100.0, 20.0);
        assert_eq!(left.align, IslandAlign::Left);
        let (x, _) = window_origin(AREA, 1000.0, 200.0, 1.0, L, left);
        assert_eq!(x, 0.0); // острів повернувся на 10 px від краю
    }

    #[test]
    fn chooses_lyrics_side() {
        assert_eq!(
            lyrics_side(AREA, 1.0, L, IslandPosition::default()),
            Side::Left
        );
        let left = position(IslandEdge::Top, IslandAlign::Left, -0.49);
        assert_eq!(lyrics_side(AREA, 1.0, L, left), Side::Right);
        let near_left = position(IslandEdge::Top, IslandAlign::Center, -0.35);
        assert_eq!(lyrics_side(AREA, 1.0, L, near_left), Side::Right); // центр на 288 px
        let right = position(IslandEdge::Top, IslandAlign::Right, 0.49);
        assert_eq!(lyrics_side(AREA, 1.0, L, right), Side::Left);
    }

    #[test]
    fn window_grows_with_sizes() {
        // За замовчуванням — як до кроку 18
        assert_eq!(Layout::default().window_size(), (1000.0, 216.0));
        // 130 % і широкий текст пісні: 2 × ((190 × 1.3 + 10) + ((420 + 10) × 1.3 + 10)) = 2 × (257 + 569) = 1652
        let big = layout_from(Some(130.0), Some(420.0));
        assert_eq!(big.window_size(), (1652.0, 281.0)); // 216 × 1.3 = 280.8 → 281
    }

    #[test]
    fn clamps_size_settings() {
        let layout = layout_from(Some(500.0), Some(10.0));
        assert_eq!(layout.island_scale, 1.3);
        assert_eq!(layout.lyrics_width, 220.0);
        assert_eq!(layout_from(None, None), Layout::default());
    }

    #[test]
    fn bigger_island_keeps_further_from_edge() {
        // При 130 % центр розгорнутого острова не ближче 190 × 1.3 + 10 = 257 px
        let big = layout_from(Some(130.0), None);
        let far_left = position(IslandEdge::Top, IslandAlign::Center, -0.5);
        let (x, _) = window_origin(AREA, 1652.0, 260.0, 1.0, big, far_left);
        assert_eq!(x, 257.0 - 826.0); // центр 257, половина вікна 826
    }
}
