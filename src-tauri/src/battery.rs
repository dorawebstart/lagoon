// battery.rs — заряд батареї ноутбука і скільки часу лишилось.
//
// Звідки дані (все від самої Windows, без інтернету):
//   - відсоток, «від мережі чи від батареї» і скільки ще протримається —
//     GetSystemPowerStatus (ті самі числа, що показує значок батареї в треї);
//   - скільки до повного заряду Windows не каже — рахуємо самі з WinRT-звіту
//     Battery::AggregateBattery(): (повна ємність − залишок) ÷ швидкість заряду;
//   - коли щось змінилося — Windows сама викликає наш обробник події
//     ReportUpdated. Ніякого опитування в циклі: поки нічого не змінюється,
//     код не працює зовсім.
//
// Якщо батареї немає (стаціонарний ПК) — нічого не надсилаємо, острів її не показує.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use windows::Devices::Power::Battery;
use windows::Foundation::TypedEventHandler;
use windows::System::Power::BatteryStatus;
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::core::IInspectable;

/// Подія для фронтенду: `BatteryInfo` або `null` (батареї немає).
pub const UPDATE_EVENT: &str = "battery:update";

/// Що бачить фронтенд. У JSON: `{ percent, plugged, charging, minutes }`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BatteryInfo {
    /// Заряд, 0–100 %.
    pub percent: u8,
    /// Під'єднано зарядку (живлення від мережі).
    pub plugged: bool,
    /// Саме зараз заряджається (під'єднано і ще не повна).
    pub charging: bool,
    /// Хвилини: від батареї — скільки ще протримається, на зарядці — скільки
    /// до повного. `None`, якщо Windows ще не порахувала (одразу після
    /// під'єднання / від'єднання).
    pub minutes: Option<u32>,
}

/// Спільний стан: батарея (її треба тримати, інакше Windows забуде про наш
/// обробник), згладжена швидкість заряду і останнє надіслане значення.
#[derive(Default)]
pub struct BatteryState {
    battery: Mutex<Option<Battery>>,
    /// Швидкість заряду в мВт, згладжена: миттєва «стрибає» щосекунди.
    charge_rate: Mutex<Option<f64>>,
    last: Mutex<Option<BatteryInfo>>,
}

/// `invoke("battery_current")` → `BatteryInfo | null`.
#[tauri::command]
pub fn battery_current(state: State<BatteryState>) -> Option<BatteryInfo> {
    state.last.lock().unwrap().clone()
}

/// Підписується на зміни батареї і надсилає перший стан.
pub fn start(app: &AppHandle) {
    // Батареї немає — нічого не робимо (і нічого не підписуємо)
    if read_power_status().is_none() {
        return;
    }
    refresh(app);

    // AggregateBattery — «усі батареї разом» (у деяких ноутбуків їх дві)
    let Ok(battery) = Battery::AggregateBattery() else {
        return;
    };
    let handle = app.clone();
    // TypedEventHandler::new — обгортка над замиканням, яку Windows викличе
    // сама (з власного потоку), коли зміниться заряд чи швидкість.
    // `move` — замикання забирає свою копію `handle`. Повертає Ok(()),
    // бо Windows очікує результат «оброблено без помилок».
    let handler = TypedEventHandler::<Battery, IInspectable>::new(move |_, _| {
        refresh(&handle);
        Ok(())
    });
    if battery.ReportUpdated(&handler).is_ok() {
        *app.state::<BatteryState>().battery.lock().unwrap() = Some(battery);
    }
}

/// Читає поточний стан і надсилає фронтенду, якщо щось змінилося.
fn refresh(app: &AppHandle) {
    let state = app.state::<BatteryState>();
    let info = read_info(&state);
    let mut last = state.last.lock().unwrap();
    if *last != info {
        *last = info.clone();
        let _ = app.emit(UPDATE_EVENT, info);
    }
}

fn read_info(state: &BatteryState) -> Option<BatteryInfo> {
    let power = read_power_status()?;
    let report = read_report();
    let charging = power.plugged && report.as_ref().is_some_and(|r| r.charging);

    let minutes = if charging {
        // До повного: рахуємо самі зі згладженої швидкості заряду
        let report = report.as_ref()?;
        let rate = smooth_rate(&state.charge_rate, report.rate_mw);
        minutes_to_full(report.remaining_mwh, report.full_mwh, rate)
    } else {
        // Не заряджаємо — згладжена швидкість більше не потрібна
        *state.charge_rate.lock().unwrap() = None;
        if power.plugged {
            None
        } else {
            power.seconds_left.map(|s| s / 60)
        }
    };

    Some(BatteryInfo {
        percent: power.percent,
        plugged: power.plugged,
        charging,
        minutes,
    })
}

// ───────────── Windows ─────────────

/// Те, що нам потрібно з GetSystemPowerStatus.
struct PowerStatus {
    percent: u8,
    plugged: bool,
    seconds_left: Option<u32>,
}

/// `None` — батареї немає або Windows не знає заряду.
fn read_power_status() -> Option<PowerStatus> {
    let mut status = SYSTEM_POWER_STATUS::default();
    // unsafe: функція Windows пише в нашу структуру за вказівником.
    // `&mut status` — даємо їй тимчасовий доступ на запис (borrow).
    unsafe { GetSystemPowerStatus(&mut status) }.ok()?;
    parse_power_status(&status)
}

/// Розбір окремо від виклику Windows — щоб перевірити тестами.
fn parse_power_status(status: &SYSTEM_POWER_STATUS) -> Option<PowerStatus> {
    // BatteryFlag 128 — «батареї немає», 255 — «невідомо»; відсоток 255 — невідомо
    if status.BatteryFlag == 128 || status.BatteryFlag == 255 || status.BatteryLifePercent > 100 {
        return None;
    }
    Some(PowerStatus {
        percent: status.BatteryLifePercent,
        plugged: status.ACLineStatus == 1,
        // u32::MAX (0xFFFFFFFF) — Windows ще не порахувала
        seconds_left: (status.BatteryLifeTime != u32::MAX).then_some(status.BatteryLifeTime),
    })
}

/// Те, що нам потрібно з WinRT-звіту батареї.
struct Report {
    charging: bool,
    rate_mw: Option<i32>,
    remaining_mwh: Option<i32>,
    full_mwh: Option<i32>,
}

fn read_report() -> Option<Report> {
    let report = Battery::AggregateBattery().ok()?.GetReport().ok()?;
    // Числа приходять як IReference<i32> — «число, якого може не бути».
    // `.ok()` після кожного кроку: немає значення → None.
    let number = |value: windows::core::Result<windows::Foundation::IReference<i32>>| {
        value.ok().and_then(|reference| reference.Value().ok())
    };
    Some(Report {
        charging: report.Status().ok() == Some(BatteryStatus::Charging),
        rate_mw: number(report.ChargeRateInMilliwatts()),
        remaining_mwh: number(report.RemainingCapacityInMilliwattHours()),
        full_mwh: number(report.FullChargeCapacityInMilliwattHours()),
    })
}

// ───────────── Розрахунки ─────────────

/// Згладжує швидкість заряду: 70 % старого значення + 30 % нового —
/// так час «до повного» не стрибає щоразу, коли Windows оновлює звіт.
fn smooth_rate(stored: &Mutex<Option<f64>>, rate_mw: Option<i32>) -> Option<f64> {
    let mut stored = stored.lock().unwrap();
    // Додатна швидкість — заряджаємо; 0 чи від'ємна — нічого не міняємо
    if let Some(rate) = rate_mw.filter(|rate| *rate > 0) {
        let rate = f64::from(rate);
        *stored = Some(match *stored {
            Some(previous) => previous * 0.7 + rate * 0.3,
            None => rate,
        });
    }
    *stored
}

/// Хвилин до повного: (повна ємність − залишок) ÷ швидкість, у годинах × 60.
fn minutes_to_full(
    remaining_mwh: Option<i32>,
    full_mwh: Option<i32>,
    rate_mw: Option<f64>,
) -> Option<u32> {
    let missing = f64::from(full_mwh? - remaining_mwh?).max(0.0);
    let rate = rate_mw.filter(|rate| *rate > 0.0)?;
    Some((missing / rate * 60.0).round() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(flag: u8, percent: u8, ac: u8, seconds: u32) -> SYSTEM_POWER_STATUS {
        SYSTEM_POWER_STATUS {
            ACLineStatus: ac,
            BatteryFlag: flag,
            BatteryLifePercent: percent,
            BatteryLifeTime: seconds,
            ..Default::default()
        }
    }

    #[test]
    fn no_battery_is_none() {
        assert!(parse_power_status(&status(128, 255, 1, u32::MAX)).is_none());
        assert!(parse_power_status(&status(255, 50, 1, 0)).is_none());
        assert!(parse_power_status(&status(1, 255, 0, 0)).is_none());
    }

    #[test]
    fn reads_percent_and_time() {
        let power = parse_power_status(&status(1, 61, 0, 15_082)).unwrap();
        assert_eq!(power.percent, 61);
        assert!(!power.plugged);
        assert_eq!(power.seconds_left, Some(15_082));

        let power = parse_power_status(&status(8, 80, 1, u32::MAX)).unwrap();
        assert!(power.plugged);
        assert_eq!(power.seconds_left, None);
    }

    #[test]
    fn minutes_to_full_from_rate() {
        // Бракує 20 000 мВт·год, заряджаємо 20 000 мВт → 1 година
        assert_eq!(
            minutes_to_full(Some(30_000), Some(50_000), Some(20_000.0)),
            Some(60)
        );
        // Невідома швидкість або повна батарея
        assert_eq!(minutes_to_full(Some(30_000), Some(50_000), None), None);
        assert_eq!(
            minutes_to_full(Some(50_000), Some(50_000), Some(10_000.0)),
            Some(0)
        );
    }

    #[test]
    fn rate_is_smoothed() {
        let stored = Mutex::new(None);
        assert_eq!(smooth_rate(&stored, Some(10_000)), Some(10_000.0));
        // 10 000 × 0.7 + 20 000 × 0.3 = 13 000
        assert_eq!(smooth_rate(&stored, Some(20_000)), Some(13_000.0));
        // Від'ємна (розряд) або невідома — лишаємо попередню
        assert_eq!(smooth_rate(&stored, Some(-5_000)), Some(13_000.0));
        assert_eq!(smooth_rate(&stored, None), Some(13_000.0));
    }

    /// Справжня батарея цього комп'ютера: `cargo test live_battery -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_battery() {
        let state = BatteryState::default();
        println!("батарея: {:?}", read_info(&state));
    }
}
