// keyboard.rs — розкладка клавіатури (мова вводу) активного вікна.
//
// Коли користувач перемикає мову (Win+Пробіл, Alt+Shift), острів на 1,5 с
// показує «UA» / «EN». Як дізнаємось:
//   GetForegroundWindow   — вікно, в якому зараз друкують;
//   GetWindowThreadProcessId — потік цього вікна;
//   GetKeyboardLayout     — розкладка цього потоку (молодші 16 біт — код мови);
//   LCIDToLocaleName      — код мови → "uk-UA" → показуємо "UA".
//
// Приватність: ми лише ЧИТАЄМО, яка мова ввімкнена. Натискання клавіш не
// перехоплюємо і не бачимо (жодних keyboard hooks).
//
// Окремий потік раз на 250 мс порівнює мову з попередньою і надсилає подію
// лише при зміні. Поки острів схований — перевіряємо рідко.

use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use windows::Win32::Globalization::LCIDToLocaleName;
use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use crate::visibility;

/// Подія для фронтенду: коротка назва мови, наприклад "UA".
pub const LAYOUT_EVENT: &str = "keyboard:layout";
/// Як часто перевіряти мову, поки острів видно.
const POLL: Duration = Duration::from_millis(250);
/// Поки острів схований — рідко (лише щоб не відстати від правди).
const HIDDEN_POLL: Duration = Duration::from_secs(1);
/// Найдовша назва мовного стандарту Windows (LOCALE_NAME_MAX_LENGTH).
const LOCALE_NAME_MAX: usize = 85;

/// Запускає фоновий потік, що стежить за мовою вводу.
pub fn start(app: AppHandle) {
    thread::spawn(move || {
        // Мова на старті: запам'ятовуємо, але не показуємо
        let mut last = current_language();
        loop {
            if !visibility::is_visible(&app) {
                thread::sleep(HIDDEN_POLL);
                // Острів знову покажуть — без «спалаху» за пропущене
                last = current_language();
                continue;
            }
            let now = current_language();
            if now.is_some() && now != last {
                if let Some(name) = &now {
                    let _ = app.emit(LAYOUT_EVENT, name);
                }
                last = now;
            }
            thread::sleep(POLL);
        }
    });
}

/// Коротка назва мови вводу активного вікна ("UA", "EN"…) або `None`,
/// якщо активного вікна немає (наприклад, робочий стіл блокування).
fn current_language() -> Option<String> {
    // unsafe: виклики Windows API. Вони нічого не пишуть у нашу пам'ять,
    // окрім буфера для назви, розмір якого ми передаємо.
    unsafe {
        let window = GetForegroundWindow();
        if window.0.is_null() {
            return None;
        }
        // None — номер процесу нам не потрібен, лише потік
        let thread_id = GetWindowThreadProcessId(window, None);
        let layout = GetKeyboardLayout(thread_id);
        // HKL — «ручка» розкладки; молодші 16 біт — код мови (LANGID).
        // `as usize` — дивимось на вказівник як на число; `& 0xFFFF` — молодші біти.
        let language_id = (layout.0 as usize & 0xFFFF) as u32;
        if language_id == 0 {
            return None;
        }
        let mut buffer = [0u16; LOCALE_NAME_MAX];
        // Повертає кількість символів разом з нульовим у кінці; 0 — помилка
        let length = LCIDToLocaleName(language_id, Some(&mut buffer), 0);
        if length <= 1 {
            return None;
        }
        let locale = String::from_utf16_lossy(&buffer[..(length - 1) as usize]);
        Some(short_name(&locale))
    }
}

/// "uk-UA" → "UA", "en-US" → "EN", "pl-PL" → "PL".
/// Українську показуємо як «UA» (так її звикли бачити), решту — кодом мови.
fn short_name(locale: &str) -> String {
    // split('-').next() — частина до першого дефіса ("uk"); unwrap_or — якщо порожньо
    let language = locale.split('-').next().unwrap_or(locale);
    match language {
        "uk" => "UA".to_string(),
        other => other.to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_names() {
        assert_eq!(short_name("uk-UA"), "UA");
        assert_eq!(short_name("en-US"), "EN");
        assert_eq!(short_name("en-GB"), "EN");
        assert_eq!(short_name("pl-PL"), "PL");
        assert_eq!(short_name("de"), "DE");
    }

    /// Мова вводу активного вікна: `cargo test live_layout -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_layout() {
        println!("мова: {:?}", current_language());
    }
}
