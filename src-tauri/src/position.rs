// position.rs — де стоїть острів: вгорі чи внизу екрана і наскільки зсунутий
// вліво/вправо. Тут же — перетягування острова мишею і «прилипання» до краю.
//
// Позиція зберігається в settings.json двома полями:
//   islandEdge   — "top" | "bottom";
//   islandOffset — зсув центру острова від центру робочої області
//                  у частках її ширини (−0.5…0.5). Частки, а не пікселі, —
//                  щоб позиція не «з'їжджала» після зміни роздільної здатності.
//
// Усе рахуємо від робочої області основного монітора (екран мінус панель
// задач), тож унизу острів стоїть над панеллю задач, а не під нею.
//
// Перетягування: фронтенд (drag.js) лише повідомляє «почали тягнути»
// (команда island_drag_start). Далі потік, який і так стежить за курсором
// (window.rs), кожні 10 мс зсуває вікно за курсором, а коли ліва кнопка миші
// відпущена — «прилипає» до найближчого краю, зберігає позицію і надсилає
// фронтенду подію island:drag-end.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewWindow};
use tauri_plugin_store::StoreExt;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};

use crate::settings::STORE_FILE;

/// Подія для фронтенду: де острів (край) і з якого боку текст пісні.
pub const PLACEMENT_EVENT: &str = "island:placement";
/// Подія для фронтенду: перетягування завершилося.
pub const DRAG_END_EVENT: &str = "island:drag-end";

/// Скільки місця (CSS px) має лишатися від центру острова до краю екрана:
/// половина розгорнутого острова (380 / 2) + 10 px відступу.
const ISLAND_HALF_WIDTH: f64 = 200.0;
/// Скільки місця (CSS px) займає острівець з текстом пісні разом із проміжком:
/// 280 + 10 + 10 px відступу від краю екрана.
const LYRICS_SPACE: f64 = 300.0;

/// Край екрана. `#[serde(rename_all = "lowercase")]` — у JSON це "top" / "bottom".
/// `#[default]` — значення для `IslandEdge::default()` (верх).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IslandEdge {
    #[default]
    Top,
    Bottom,
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
    /// Зсув центру від центру робочої області, у частках її ширини.
    pub offset: f64,
}

/// Що отримує фронтенд у події island:placement і з команди island_placement.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    pub edge: IslandEdge,
    pub lyrics_side: Side,
}

/// Прямокутник робочої області у фізичних пікселях екрана.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

// ───────── Чисті розрахунки (без Windows, тому їх легко тестувати) ─────────

/// Обмежує центр острова так, щоб розгорнутий острів не виходив за екран.
/// `scale` — масштаб Windows (1.25 при 125 %): CSS px × scale = фізичні px.
fn clamp_center(area: Area, scale: f64, center_x: f64) -> f64 {
    let margin = ISLAND_HALF_WIDTH * scale;
    let min = area.x + margin;
    let max = area.x + area.width - margin;
    if min > max {
        // Екран вужчий за острів (майже неможливо) — просто центр.
        area.x + area.width / 2.0
    } else {
        center_x.clamp(min, max)
    }
}

/// Лівий верхній кут вікна для позиції `position`.
/// Центр острова збігається з центром вікна (CSS центрує його по горизонталі).
pub fn window_origin(
    area: Area,
    window_width: f64,
    window_height: f64,
    scale: f64,
    position: IslandPosition,
) -> (f64, f64) {
    let wanted = area.x + area.width / 2.0 + position.offset * area.width;
    let center = clamp_center(area, scale, wanted);
    let x = center - window_width / 2.0;
    let y = match position.edge {
        IslandEdge::Top => area.y,
        // Унизу вікно прив'язане нижнім краєм до низу робочої області.
        IslandEdge::Bottom => area.y + area.height - window_height,
    };
    (x, y)
}

/// «Прилипання» після перетягування: край — за тим, у якій половині екрана
/// курсор; зсув — за тим, де зараз центр вікна (з обмеженням межами екрана).
pub fn snap(area: Area, scale: f64, window_center_x: f64, cursor_y: f64) -> IslandPosition {
    let center = clamp_center(area, scale, window_center_x);
    let edge = if cursor_y < area.y + area.height / 2.0 {
        IslandEdge::Top
    } else {
        IslandEdge::Bottom
    };
    IslandPosition {
        edge,
        offset: (center - (area.x + area.width / 2.0)) / area.width,
    }
}

/// Текст пісні — ліворуч від острова, а якщо там не вміщається (острів біля
/// лівого краю екрана) — праворуч.
pub fn lyrics_side(area: Area, scale: f64, position: IslandPosition) -> Side {
    let wanted = area.x + area.width / 2.0 + position.offset * area.width;
    let center = clamp_center(area, scale, wanted);
    if center - area.x < (ISLAND_HALF_WIDTH + LYRICS_SPACE) * scale {
        Side::Right
    } else {
        Side::Left
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
    let offset = store
        .get("islandOffset")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    IslandPosition { edge, offset }
}

/// Записує позицію в settings.json (той самий файл, що й у JS).
fn save(app: &AppHandle, position: IslandPosition) {
    if let Ok(store) = app.store(STORE_FILE) {
        store.set("islandEdge", json!(position.edge));
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
    let size = window.outer_size()?;
    let (x, y) = window_origin(area, size.width as f64, size.height as f64, scale, position);
    // `as i32` — координати вікна цілі; дробову частину просто відкидаємо.
    window.set_position(PhysicalPosition::new(x as i32, y as i32))?;
    let _ = window.emit(PLACEMENT_EVENT, placement(area, scale, position));
    Ok(())
}

fn placement(area: Area, scale: f64, position: IslandPosition) -> Placement {
    Placement {
        edge: position.edge,
        lyrics_side: lyrics_side(area, scale, position),
    }
}

/// Команда для JS: `invoke("island_placement")` — де острів зараз.
/// Потрібна при завантаженні сторінки: подія могла прийти раніше, ніж сторінка підписалася.
///
/// Параметр `window: WebviewWindow` Tauri підставляє сам — це вікно, яке викликало команду.
#[tauri::command]
pub fn island_placement(app: AppHandle, window: WebviewWindow) -> Option<Placement> {
    let (area, scale) = primary_area(&window)?;
    Some(placement(area, scale, saved(&app)))
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
    let (Ok(origin), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        return;
    };
    let center_x = origin.x as f64 + size.width as f64 / 2.0;
    let position = snap(area, scale, center_x, cursor_y);
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

    #[test]
    fn places_top_center_by_default() {
        let (x, y) = window_origin(AREA, 1000.0, 200.0, 1.0, IslandPosition::default());
        assert_eq!((x, y), (460.0, 0.0));
    }

    #[test]
    fn places_bottom_above_taskbar() {
        let position = IslandPosition {
            edge: IslandEdge::Bottom,
            offset: 0.0,
        };
        let (_, y) = window_origin(AREA, 1000.0, 200.0, 1.0, position);
        assert_eq!(y, 840.0); // 1040 − 200: нижній край вікна на межі панелі задач
    }

    #[test]
    fn clamps_offset_to_screen() {
        let position = IslandPosition {
            edge: IslandEdge::Top,
            offset: -0.5, // крайній лівий
        };
        let (x, _) = window_origin(AREA, 1000.0, 200.0, 1.0, position);
        // Центр острова не ближче 200 px до краю → вікно 1000 px починається з −300.
        assert_eq!(x, -300.0);
    }

    #[test]
    fn snaps_to_nearest_edge() {
        let top = snap(AREA, 1.0, 960.0, 300.0);
        assert_eq!(top.edge, IslandEdge::Top);
        assert_eq!(top.offset, 0.0);

        let bottom = snap(AREA, 1.0, 1440.0, 900.0);
        assert_eq!(bottom.edge, IslandEdge::Bottom);
        assert_eq!(bottom.offset, 0.25); // (1440 − 960) / 1920
    }

    #[test]
    fn moves_lyrics_right_near_left_edge() {
        let centered = IslandPosition::default();
        assert_eq!(lyrics_side(AREA, 1.0, centered), Side::Left);

        let left = IslandPosition {
            edge: IslandEdge::Top,
            offset: -0.4, // центр на 192 px → обмежиться до 200 px від краю
        };
        assert_eq!(lyrics_side(AREA, 1.0, left), Side::Right);
    }
}
