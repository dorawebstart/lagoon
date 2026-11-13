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
        // Мова на старті: запам'ятовуємо, але не показуємо.
        // Порівнюємо лише числовий код мови — це дешево; назву ("UA")
        // шукаємо тільки тоді, коли код змінився.
        let mut last = current_language_id();
        loop {
            if !visibility::is_visible(&app) {
                thread::sleep(HIDDEN_POLL);
                // Острів знову покажуть — без «спалаху» за пропущене
                last = current_language_id();
                continue;
            }
            let now = current_language_id();
            if now.is_some() && now != last {
                if let Some(name) = now.and_then(language_name) {
                    let _ = app.emit(LAYOUT_EVENT, name);
                }
                last = now;
            }
            thread::sleep(POLL);
        }
    });
}

/// Код мови вводу (LANGID) активного вікна або `None`, якщо активного вікна
/// немає (наприклад, екран блокування).
fn current_language_id() -> Option<u32> {
    // unsafe: виклики Windows API; вони лише читають стан системи.
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
        (language_id != 0).then_some(language_id)
    }
}

/// Код мови → коротка назва ("UA", "EN"…).
fn language_name(language_id: u32) -> Option<String> {
    let mut buffer = [0u16; LOCALE_NAME_MAX];
    // unsafe: Windows пише назву в наш буфер; його розмір вона знає зі зрізу.
    // Повертає кількість символів разом з нульовим у кінці; 0 — помилка.
    let length = unsafe { LCIDToLocaleName(language_id, Some(&mut buffer), 0) };
    if length <= 1 {
        return None;
    }
    let locale = String::from_utf16_lossy(&buffer[..(length - 1) as usize]);
    Some(short_name(&locale))
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
        println!("мова: {:?}", current_language_id().and_then(language_name));
    }
}
