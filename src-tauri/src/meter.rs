// meter.rs — рівень звуку програми, що грає, для «хвилі» вздовж краю острова.
//
// Windows для кожної аудіосесії рахує «пікове значення» — наскільки голосно
// звучить програма саме зараз (0.0 — тиша, 1.0 — максимум). Це той самий
// індикатор, що стрибає в «Мікшері гучності». Записувати звук не треба.
//
// Окремий потік ~30 разів на секунду читає цей рівень і надсилає фронтенду
// подію "audio:level". Працює лише коли хвиля увімкнена в налаштуваннях
// (команда audio_wave) і музика грає — інакше потік спить і нічого не шле.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, State};
use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
use windows::Win32::System::Com::CLSCTX_ALL;
use windows::core::Result as WinResult;

use crate::audio::{ComGuard, app_sessions, default_device};
use crate::media::MediaStore;

/// Подія для фронтенду: рівень звуку 0.0–1.0.
pub const LEVEL_EVENT: &str = "audio:level";
/// Як часто читати рівень, поки грає музика (~30 разів на секунду).
const FRAME: Duration = Duration::from_millis(33);
/// Як часто перевіряти, чи не почала грати музика, коли хвиля «спить».
const IDLE: Duration = Duration::from_millis(400);
/// Як часто шукати аудіосесії заново (плеєр міг відкрити нову сесію).
const REFRESH: Duration = Duration::from_secs(2);

/// Чи увімкнена хвиля в налаштуваннях. `AtomicBool` — булеве значення, яке
/// можна безпечно читати й писати з різних потоків без `Mutex`.
#[derive(Default)]
pub struct WaveEnabled(AtomicBool);

/// Команда для JS: `invoke("audio_wave", { enabled: true })`.
#[tauri::command]
pub fn audio_wave(enabled: bool, state: State<WaveEnabled>) {
    // Ordering::Relaxed — нам не важливий порядок відносно інших змінних,
    // лише саме значення.
    state.0.store(enabled, Ordering::Relaxed);
}

/// Запускає фоновий потік вимірювання рівня.
pub fn start(app: AppHandle) {
    thread::spawn(move || run(&app));
}

fn run(app: &AppHandle) {
    // COM для цього потоку — один раз на весь час його роботи.
    let _com = ComGuard::init();

    // Аудіосесії плеєра та для якої програми їх шукали.
    let mut meters: Vec<IAudioMeterInformation> = Vec::new();
    let mut meters_for: Option<String> = None;
    let mut refreshed_at: Option<Instant> = None;
    // Останній надісланий рівень: тишу (0) не шлемо 30 разів на секунду.
    let mut last_sent: Option<f32> = None;

    loop {
        let enabled = app.state::<WaveEnabled>().0.load(Ordering::Relaxed);
        let media = app.state::<MediaStore>();

        if !enabled || !media.is_playing() {
            // Щойно перестали — один раз кажемо фронтенду «тиша».
            if last_sent.is_some_and(|level| level > 0.0) {
                let _ = app.emit(LEVEL_EVENT, 0.0_f32);
            }
            last_sent = None;
            meters.clear();
            meters_for = None;
            thread::sleep(IDLE);
            continue;
        }

        // Змінилася програма або давно не шукали — шукаємо сесії заново.
        let app_id = media.current_app_id();
        let stale = refreshed_at.is_none_or(|time| time.elapsed() >= REFRESH);
        if app_id != meters_for || stale {
            meters = find_meters(app_id.as_deref()).unwrap_or_default();
            meters_for = app_id;
            refreshed_at = Some(Instant::now());
        }

        let level = peak(&meters);
        // Дві тиші поспіль не шлемо — фронтенду нічого не треба перемальовувати.
        if !(level == 0.0 && last_sent == Some(0.0)) {
            let _ = app.emit(LEVEL_EVENT, level);
            last_sent = Some(level);
        }
        thread::sleep(FRAME);
    }
}

/// Найгучніша з сесій (у браузера їх кілька), округлена до сотих.
fn peak(meters: &[IAudioMeterInformation]) -> f32 {
    let level = meters
        .iter()
        // filter_map — пропускаємо сесії, які не відповіли (Err → None)
        .filter_map(|meter| unsafe { meter.GetPeakValue() }.ok())
        // fold — «згортаємо» список в одне значення: найбільше
        .fold(0.0_f32, f32::max);
    (level * 100.0).round() / 100.0
}

/// Рівнеміри аудіосесій плеєра; не знайшли — загальний рівень динаміків.
fn find_meters(app_id: Option<&str>) -> WinResult<Vec<IAudioMeterInformation>> {
    let device = default_device()?;
    if let Some(app_id) = app_id {
        // ::<IAudioMeterInformation> — той самий пошук сесій, що й для
        // гучності (audio.rs), але просимо в кожної сесії рівнемір.
        let found = app_sessions::<IAudioMeterInformation>(&device, app_id)?;
        if !found.is_empty() {
            return Ok(found);
        }
    }
    Ok(vec![unsafe { device.Activate(CLSCTX_ALL, None)? }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peak_of_no_sessions_is_silence() {
        assert_eq!(peak(&[]), 0.0);
    }
}
