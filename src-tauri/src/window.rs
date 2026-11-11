// window.rs — усе, що стосується вікна-острова: позиція на екрані,
// а згодом click-through і зміна розміру.

use tauri::{PhysicalPosition, WebviewWindow};

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
