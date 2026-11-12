// audio.rs — гучність програми, що грає (Spotify, браузер, AIMP…).
//
// Джерело: Windows Core Audio — той самий API, яким користується «Мікшер
// гучності» Windows. Кожна програма, що відтворює звук, має там свою
// «аудіосесію» з власним повзунком. Ми знаходимо сесію плеєра і змінюємо
// гучність саме її, а не всієї системи.
//
// Як знайти потрібну сесію:
//   GSMTC (media.rs) каже, яка програма грає: `app_id`, наприклад
//   "Spotify.exe", "Chrome", "MSEdge". Перебираємо аудіосесії, для кожної
//   дізнаємося процес (PID → ім'я .exe) і порівнюємо з `app_id`.
//   Не знайшли (наприклад, програма ще не видала жодного звуку) — керуємо
//   загальною гучністю системи і кажемо фронтенду, що це «Система».
//
// Core Audio — це COM-інтерфейси (старіша за WinRT технологія Windows).
// Майже кожен виклик COM у крейті `windows` позначено `unsafe`: Rust не може
// перевірити, що робить код поза ним, тож ми беремо відповідальність на себе.

use serde::Serialize;
use tauri::State;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    AudioSessionStateExpired, IAudioSessionControl2, IAudioSessionManager2, IMMDevice,
    IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator, eMultimedia, eRender,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::core::{Interface, PWSTR, Result as WinResult};

use crate::media::MediaStore;

/// Чиєю гучністю керуємо. У JSON стане рядком "app" або "system".
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VolumeScope {
    App,
    System,
}

/// Те, що отримує фронтенд: `{ level: 0.42, scope: "app" }`.
#[derive(Serialize)]
pub struct VolumeState {
    /// Гучність від 0.0 до 1.0 (так її рахує Windows; відсотки — на фронтенді).
    pub level: f32,
    pub scope: VolumeScope,
}

/// Команда для JS: `invoke("audio_volume")` — поточна гучність плеєра.
///
/// `async` — Tauri виконає її у фоновому потоці, а не в головному потоці
/// вікна (там COM уже налаштований інакше, і ми б йому заважали).
/// `State<'_, MediaStore>` — позичаємо (borrow) спільний стан медіа, щоб
/// дізнатися, яка програма грає; `'_` — «позика живе, поки триває виклик».
#[tauri::command]
pub async fn audio_volume(media: State<'_, MediaStore>) -> Result<VolumeState, String> {
    let app_id = media.current_app_id();
    let _com = ComGuard::init();
    let target = find_target(app_id.as_deref()).map_err(|e| e.to_string())?;
    target.state().map_err(|e| e.to_string())
}

/// Команда для JS: `invoke("audio_set_volume", { level: 0.5 })`.
/// Повертає гучність, яка вийшла після зміни.
#[tauri::command]
pub async fn audio_set_volume(
    level: f32,
    media: State<'_, MediaStore>,
) -> Result<VolumeState, String> {
    // clamp обмежує число межами: -0.2 → 0.0, 1.7 → 1.0.
    let level = level.clamp(0.0, 1.0);
    let app_id = media.current_app_id();
    let _com = ComGuard::init();
    let target = find_target(app_id.as_deref()).map_err(|e| e.to_string())?;
    target.set_level(level).map_err(|e| e.to_string())?;
    target.state().map_err(|e| e.to_string())
}

/// Ініціалізує COM у поточному потоці і «прибирає» за собою.
///
/// Це патерн RAII: ресурс захоплюємо при створенні значення, а звільняємо
/// в `Drop` — Rust сам викличе `drop`, коли змінна `_com` вийде з області
/// видимості (кінець функції), навіть якщо ми вийшли раніше через `?`.
struct ComGuard {
    /// true — ініціалізація вдалася, тож треба викликати CoUninitialize.
    initialized: bool,
}

impl ComGuard {
    fn init() -> Self {
        // Якщо потік уже налаштований під COM в іншому режимі, Windows поверне
        // помилку RPC_E_CHANGED_MODE — тоді COM уже працює, і прибирати нам нічого.
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        ComGuard {
            initialized: result.is_ok(),
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { CoUninitialize() };
        }
    }
}

/// Куди спрямувати зміну гучності.
enum Target {
    /// Аудіосесії програми-плеєра. Їх може бути кілька: у браузера звук іде
    /// з кількох процесів, тож міняємо всі одразу.
    App(Vec<ISimpleAudioVolume>),
    /// Загальна гучність динаміків (запасний варіант).
    System(IAudioEndpointVolume),
}

impl Target {
    /// Поточна гучність. Для програми беремо першу сесію — вони однакові.
    fn level(&self) -> WinResult<f32> {
        match self {
            Target::App(sessions) => unsafe { sessions[0].GetMasterVolume() },
            Target::System(endpoint) => unsafe { endpoint.GetMasterVolumeLevelScalar() },
        }
    }

    fn set_level(&self, level: f32) -> WinResult<()> {
        // std::ptr::null() — «немає контексту події»: нам не треба відрізняти
        // свої зміни від чужих.
        match self {
            Target::App(sessions) => {
                for session in sessions {
                    unsafe { session.SetMasterVolume(level, std::ptr::null())? };
                }
                Ok(())
            }
            Target::System(endpoint) => unsafe {
                endpoint.SetMasterVolumeLevelScalar(level, std::ptr::null())
            },
        }
    }

    fn state(&self) -> WinResult<VolumeState> {
        let scope = match self {
            Target::App(_) => VolumeScope::App,
            Target::System(_) => VolumeScope::System,
        };
        Ok(VolumeState {
            level: self.level()?,
            scope,
        })
    }
}

/// Шукає аудіосесії плеєра; якщо не знайшли — повертає гучність системи.
fn find_target(app_id: Option<&str>) -> WinResult<Target> {
    // Пристрій виводу звуку за замовчуванням (динаміки чи навушники).
    // CoCreateInstance створює COM-об'єкт за його «ідентифікатором класу».
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
    let device = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)? };

    if let Some(app_id) = app_id {
        let sessions = app_sessions(&device, app_id)?;
        // is_empty() — у векторі немає жодної сесії.
        if !sessions.is_empty() {
            return Ok(Target::App(sessions));
        }
    }

    // Activate::<T> — «дай мені інтерфейс T цього пристрою». Тип T Rust
    // виводить сам із типу, який ми чекаємо (IAudioEndpointVolume).
    let endpoint = unsafe { device.Activate(CLSCTX_ALL, None)? };
    Ok(Target::System(endpoint))
}

/// Усі живі аудіосесії, процес яких відповідає `app_id`.
fn app_sessions(device: &IMMDevice, app_id: &str) -> WinResult<Vec<ISimpleAudioVolume>> {
    let manager: IAudioSessionManager2 = unsafe { device.Activate(CLSCTX_ALL, None)? };
    let sessions = unsafe { manager.GetSessionEnumerator()? };
    let count = unsafe { sessions.GetCount()? };

    let wanted = normalize_app_id(app_id);
    let mut found = Vec::new();

    for index in 0..count {
        let control = unsafe { sessions.GetSession(index)? };
        // cast() — запитуємо в COM-об'єкта інший інтерфейс (QueryInterface).
        // IAudioSessionControl2 уміє повідомити PID процесу.
        let control: IAudioSessionControl2 = control.cast()?;

        // Сесія закритої програми ще може висіти в списку — пропускаємо.
        if unsafe { control.GetState()? } == AudioSessionStateExpired {
            continue;
        }
        // PID 0 — системні звуки Windows, вони нам не потрібні.
        let pid = unsafe { control.GetProcessId() }.unwrap_or(0);
        if pid == 0 {
            continue;
        }
        // `let ... else` — якщо ім'я процесу не дізналися (None), пропускаємо сесію.
        let Some(process) = process_name(pid) else {
            continue;
        };
        if app_matches(&wanted, &process) {
            found.push(control.cast::<ISimpleAudioVolume>()?);
        }
    }
    Ok(found)
}

/// Ім'я файлу процесу без шляху і ".exe", малими літерами: PID → "spotify".
fn process_name(pid: u32) -> Option<String> {
    // `.ok()?` — перетворюємо Result на Option і виходимо з None при помилці
    // (наприклад, процес чужого користувача, до якого немає доступу).
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;

    // Буфер для шляху в UTF-16 (рядки Windows API — 16-бітні).
    let mut buffer = [0u16; 1024];
    let mut size = buffer.len() as u32;
    let result = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
    };
    // Дескриптор процесу треба закрити завжди, навіть якщо запит не вдався.
    let _ = unsafe { CloseHandle(handle) };
    result.ok()?;

    // &buffer[..size] — зріз (slice): лише заповнена частина буфера.
    let path = String::from_utf16_lossy(&buffer[..size as usize]);
    let file = path.rsplit('\\').next()?;
    Some(strip_exe(&file.to_lowercase()).to_string())
}

/// Приводить `app_id` з GSMTC до вигляду імені процесу.
///
/// Приклади:
///   "Spotify.exe" → "spotify"
///   "Chrome" → "chrome"
///   "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify" (Spotify з Microsoft Store) → "spotify"
fn normalize_app_id(app_id: &str) -> String {
    let lower = app_id.to_lowercase();
    // Програми з Microsoft Store мають id «пакет!застосунок» — беремо частину після «!».
    let name = lower.rsplit('!').next().unwrap_or(&lower);
    // Відомі винятки: Firefox називає себе в GSMTC не іменем, а хешем.
    match name {
        "308046b0af4a39cb" => "firefox".to_string(),
        other => strip_exe(other).to_string(),
    }
}

/// "spotify.exe" → "spotify"; рядки без ".exe" не змінюються.
fn strip_exe(name: &str) -> &str {
    // strip_suffix повертає Option: Some(без суфікса) або None, якщо суфікса немає.
    name.strip_suffix(".exe").unwrap_or(name)
}

/// Чи належить процес `process` програмі `wanted` (обидва вже нормалізовані).
/// Окрім точного збігу, дозволяємо «один містить інший» — щоб "msedge" знайшов
/// "msedge", а "spotifymusic" — "spotify". Короткі імена (до 4 літер) — лише
/// точний збіг, інакше "app" збігся б із чим завгодно.
fn app_matches(wanted: &str, process: &str) -> bool {
    if wanted.is_empty() || process.is_empty() {
        return false;
    }
    if wanted == process {
        return true;
    }
    let long_enough = wanted.len() >= 4 && process.len() >= 4;
    long_enough && (wanted.contains(process) || process.contains(wanted))
}

// Модульні тести: `cargo test` запускає функції з #[test].
// #[cfg(test)] — цей код компілюється лише під час тестів, у програму не потрапляє.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_app_ids() {
        assert_eq!(normalize_app_id("Spotify.exe"), "spotify");
        assert_eq!(normalize_app_id("Chrome"), "chrome");
        assert_eq!(normalize_app_id("MSEdge"), "msedge");
        assert_eq!(
            normalize_app_id("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"),
            "spotify"
        );
        assert_eq!(normalize_app_id("308046B0AF4A39CB"), "firefox");
    }

    #[test]
    fn matches_processes() {
        assert!(app_matches("spotify", "spotify"));
        assert!(app_matches("chrome", "chrome"));
        assert!(app_matches("aimp", "aimp"));
        assert!(!app_matches("chrome", "msedge"));
        assert!(!app_matches("vlc", "vlcx"));
        assert!(!app_matches("", "spotify"));
    }
}
