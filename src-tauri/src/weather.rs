// weather.rs — погода з Open-Meteo (https://open-meteo.com).
//
// Open-Meteo — безкоштовний відкритий API без ключа, тож жодних секретів
// у репозиторії. Що робимо:
//   - фонова задача раз на 30 хв питає погоду для міста з налаштувань
//     (лише коли погода увімкнена, місто обране і острів видно);
//   - результат надсилаємо фронтенду подією "weather:update" і кладемо в кеш
//     на диску, щоб після перезапуску одразу показати останню погоду;
//   - погоду, старшу за 3 години, не показуємо (наприклад, немає інтернету);
//   - команда weather_search_city шукає місто за назвою (для налаштувань).
//
// Приватність: координати міста зберігаються лише в settings.json, а на
// сервер ідуть округлені до 2 знаків після коми (~1 км).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_store::StoreExt;
use tokio::sync::Notify;

use crate::{settings, visibility};

const FORECAST_API: &str = "https://api.open-meteo.com/v1/forecast";
const GEOCODING_API: &str = "https://geocoding-api.open-meteo.com/v1/search";

/// Подія для фронтенду: `Weather` або `null` (погоду вимкнули / немає міста).
pub const UPDATE_EVENT: &str = "weather:update";
/// Як часто оновлювати погоду.
const REFRESH: Duration = Duration::from_secs(30 * 60);
/// Як часто задача «прокидається» перевірити, чи не час оновити
/// (острів знову видно, змінилось місто…).
const TICK: Duration = Duration::from_secs(60);
/// Погоду, старшу за це, не показуємо.
const MAX_AGE: Duration = Duration::from_secs(3 * 60 * 60);
/// Ключі в settings.json (мають збігатися з DEFAULTS у src/settings-store.js).
const ENABLED_KEY: &str = "weatherEnabled";
const CITY_KEY: &str = "weatherCity";

/// Місто: назва для показу і координати. Так само лежить у settings.json.
///
/// `PartialEq` — дозволяє порівнювати міста через `==` (чи змінилось місто).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct City {
    pub name: String,
    /// Область / регіон (для пошуку: «Львів, Львівська область»).
    #[serde(default)]
    pub admin: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
}

/// Погода, яку отримує фронтенд (і яка лежить у кеші).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Weather {
    /// Назва міста, для якого ця погода.
    pub city: String,
    /// Температура і «відчувається як», °C.
    pub temperature: f64,
    pub apparent_temperature: f64,
    /// Код погоди WMO (0 — ясно, 3 — хмарно, 61 — дощ…); іконку обирає фронтенд.
    pub weather_code: u16,
    /// День чи ніч (сонце чи місяць на іконці).
    pub is_day: bool,
    /// Вітер: швидкість у м/с і напрямок, звідки дме (0° — північ, 90° — схід).
    pub wind_speed: f64,
    pub wind_direction: f64,
    /// Відносна вологість, %.
    pub humidity: f64,
    /// Схід і захід сонця за місцевим часом міста, "HH:MM".
    pub sunrise: String,
    pub sunset: String,
    /// Коли отримали, секунди від 1970 року (для «оновлено о…»).
    pub updated_at: u64,
}

/// Спільний стан: остання погода і «дзвіночок», щоб розбудити фонову задачу
/// одразу (змінили місто), а не чекати до наступного тіку.
///
/// `Arc<Notify>` — Notify з tokio: одна сторона `notify_one()`, інша чекає
/// `notified().await`. `Arc` — спільне володіння: задача тримає свою копію.
#[derive(Default)]
pub struct WeatherState {
    last: Mutex<Option<Weather>>,
    /// Для якого міста отримано `last` (порівнюємо всю структуру з
    /// координатами: двох Олександрій за назвою не розрізнити).
    fetched_for: Mutex<Option<City>>,
    wake: Arc<Notify>,
}

// ───────────── Команди для JS ─────────────

/// `invoke("weather_current")` → `Weather | null` — остання свіжа погода
/// (острів питає на старті, далі слухає подію).
#[tauri::command]
pub fn weather_current(app: AppHandle, state: State<WeatherState>) -> Option<Weather> {
    if !enabled(&app) {
        return None;
    }
    let last = state.last.lock().unwrap().clone();
    last.filter(|weather| is_fresh(weather, now()))
}

/// `invoke("weather_search_city", { query, language })` → список міст (до 5).
/// `language` — "uk" або "en": якою мовою повернути назви.
#[tauri::command]
pub async fn weather_search_city(query: String, language: String) -> Result<Vec<City>, String> {
    let query = query.trim();
    // Сервер шукає від 2 літер; коротший запит не має сенсу надсилати
    if query.chars().count() < 2 {
        return Ok(Vec::new());
    }
    search_city(query, &language)
        .await
        .map_err(|error| error.to_string())
}

// ───────────── Фонова задача ─────────────

/// Запускає фонову задачу оновлення погоди.
pub fn start(app: AppHandle) {
    let state = app.state::<WeatherState>();
    // Погода з кешу — щоб після перезапуску не чекати запиту до сервера
    if let Some(cached) = read_cache(&app) {
        *state.last.lock().unwrap() = Some(cached);
    }
    let wake = state.wake.clone();
    // async_runtime::spawn — запускає async-задачу в рантаймі Tauri (tokio).
    // `async move { ... }` — як async-стрілкова функція в JS, що забирає
    // `app` і `wake` всередину.
    tauri::async_runtime::spawn(async move {
        loop {
            tick(&app).await;
            // Чекаємо TICK або «дзвіночок» — що настане раніше.
            // timeout повертає Err, якщо час вийшов; нам однаково — `let _`.
            let _ = tokio::time::timeout(TICK, wake.notified()).await;
        }
    });
}

/// Налаштування погоди змінились (увімкнули, обрали місто) — оновити одразу.
pub fn settings_changed(app: &AppHandle) {
    app.state::<WeatherState>().wake.notify_one();
}

/// Одна перевірка: чи треба оновити погоду, і якщо так — запит до сервера.
async fn tick(app: &AppHandle) {
    let state = app.state::<WeatherState>();

    let city = saved_city(app);
    let Some(city) = city.filter(|_| enabled(app)) else {
        // Погоду вимкнули або місто не обране — фронтенд ховає погоду.
        // `take()` забирає значення і лишає None: подію шлемо лише раз.
        if state.last.lock().unwrap().take().is_some() {
            let _ = app.emit(UPDATE_EVENT, None::<Weather>);
        }
        return;
    };

    // Поки острів схований — сервер не питаємо
    if !visibility::is_visible(app) {
        return;
    }

    let now = now();
    let last = state.last.lock().unwrap().clone();
    // Після перезапуску погода з кешу вважається «своєю», якщо назва та сама
    let same_city = match &*state.fetched_for.lock().unwrap() {
        Some(fetched) => *fetched == city,
        None => last
            .as_ref()
            .is_some_and(|weather| weather.city == city.name),
    };
    // Оновлюємо, якщо погоди немає, вона для іншого міста або застаріла
    let due = match &last {
        Some(weather) => !same_city || now.saturating_sub(weather.updated_at) >= REFRESH.as_secs(),
        None => true,
    };
    if !due {
        return;
    }

    match fetch_forecast(&city).await {
        Ok(weather) => {
            write_cache(app, &weather);
            *state.fetched_for.lock().unwrap() = Some(city);
            *state.last.lock().unwrap() = Some(weather.clone());
            let _ = app.emit(UPDATE_EVENT, Some(weather));
        }
        Err(error) => {
            eprintln!("погода: {error}");
            // Немає інтернету: стара погода ще годиться, поки не старша
            // за 3 години; потім кажемо фронтенду її сховати.
            if last.is_some_and(|weather| !is_fresh(&weather, now)) {
                let _ = app.emit(UPDATE_EVENT, None::<Weather>);
            }
        }
    }
}

// ───────────── Налаштування ─────────────

/// Чи увімкнена погода (за замовчуванням — так).
fn enabled(app: &AppHandle) -> bool {
    let Ok(store) = app.store(settings::STORE_FILE) else {
        return true;
    };
    // `and_then` — якщо значення є, пробуємо взяти з нього bool;
    // `unwrap_or(true)` — якщо ключа немає або там не bool.
    store
        .get(ENABLED_KEY)
        .and_then(|value| value.as_bool())
        .unwrap_or(true)
}

/// Місто з налаштувань (`None`, якщо ще не обрали).
fn saved_city(app: &AppHandle) -> Option<City> {
    let store = app.store(settings::STORE_FILE).ok()?;
    // from_value перетворює довільне JSON-значення на нашу структуру.
    // `.ok()` — Result → Option: зламаний запис = «міста немає».
    serde_json::from_value(store.get(CITY_KEY)?).ok()
}

// ───────────── Запити до Open-Meteo ─────────────

/// Відповідь /v1/forecast (лише потрібні поля).
#[derive(Deserialize)]
struct ForecastResponse {
    current: CurrentWeather,
    daily: DailyWeather,
}

#[derive(Deserialize)]
struct CurrentWeather {
    temperature_2m: f64,
    apparent_temperature: f64,
    relative_humidity_2m: f64,
    weather_code: u16,
    /// Open-Meteo дає 1 / 0, а не true / false.
    is_day: u8,
    wind_speed_10m: f64,
    wind_direction_10m: f64,
}

/// Денні значення — масиви по днях; ми просимо один день, тож беремо [0].
#[derive(Deserialize)]
struct DailyWeather {
    sunrise: Vec<String>,
    sunset: Vec<String>,
}

/// Відповідь геокодера. `results` немає взагалі, якщо нічого не знайдено.
#[derive(Deserialize)]
struct GeocodingResponse {
    #[serde(default)]
    results: Vec<GeocodingPlace>,
}

#[derive(Deserialize)]
struct GeocodingPlace {
    name: String,
    latitude: f64,
    longitude: f64,
    country: Option<String>,
    admin1: Option<String>,
}

fn client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(format!(
            "Lagoon/{} ({})",
            env!("CARGO_PKG_VERSION"),
            env!("CARGO_PKG_REPOSITORY")
        ))
        .timeout(Duration::from_secs(10))
        .build()
}

async fn fetch_forecast(city: &City) -> reqwest::Result<Weather> {
    // query(...) сам кодує параметри в URL (?latitude=50.45&...)
    let response: ForecastResponse = client()?
        .get(FORECAST_API)
        .query(&[
            ("latitude", round_coordinate(city.latitude)),
            ("longitude", round_coordinate(city.longitude)),
            (
                "current",
                "temperature_2m,apparent_temperature,relative_humidity_2m,\
                 weather_code,is_day,wind_speed_10m,wind_direction_10m"
                    .to_string(),
            ),
            ("daily", "sunrise,sunset".to_string()),
            ("wind_speed_unit", "ms".to_string()),
            // Час сходу / заходу — за часовим поясом самого міста
            ("timezone", "auto".to_string()),
            ("forecast_days", "1".to_string()),
        ])
        .send()
        .await?
        // error_for_status: відповідь 4xx / 5xx — теж помилка
        .error_for_status()?
        .json()
        .await?;
    Ok(to_weather(response, &city.name, now()))
}

async fn search_city(query: &str, language: &str) -> reqwest::Result<Vec<City>> {
    let response: GeocodingResponse = client()?
        .get(GEOCODING_API)
        .query(&[
            ("name", query),
            ("count", "5"),
            ("language", language),
            ("format", "json"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    // into_iter().map(...).collect() — перетворити кожен елемент списку
    // (як array.map у JS) і зібрати в новий Vec.
    Ok(response
        .results
        .into_iter()
        .map(|place| City {
            name: place.name,
            admin: place.admin1,
            country: place.country,
            latitude: place.latitude,
            longitude: place.longitude,
        })
        .collect())
}

/// Відповідь сервера → наша структура `Weather`.
fn to_weather(response: ForecastResponse, city: &str, updated_at: u64) -> Weather {
    let current = response.current;
    // first() — Option: масив міг прийти порожнім (полярна ніч теж дає рядок,
    // але про всяк випадок) — тоді показуємо «—».
    let time_of = |list: &[String]| {
        list.first()
            .map(|value| clock_time(value))
            .unwrap_or_else(|| "—".to_string())
    };
    Weather {
        city: city.to_string(),
        temperature: current.temperature_2m,
        apparent_temperature: current.apparent_temperature,
        weather_code: current.weather_code,
        is_day: current.is_day == 1,
        wind_speed: current.wind_speed_10m,
        wind_direction: current.wind_direction_10m,
        humidity: current.relative_humidity_2m,
        sunrise: time_of(&response.daily.sunrise),
        sunset: time_of(&response.daily.sunset),
        updated_at,
    }
}

// ───────────── Допоміжне ─────────────

/// "2026-11-13T07:02" → "07:02" (беремо частину після T).
fn clock_time(value: &str) -> String {
    match value.split_once('T') {
        Some((_, time)) => time.chars().take(5).collect(),
        None => value.to_string(),
    }
}

/// Координата з точністю ~1 км: 50.4501 → "50.45".
fn round_coordinate(value: f64) -> String {
    format!("{value:.2}")
}

/// Чи погода ще не застаріла (молодша за 3 години).
fn is_fresh(weather: &Weather, now: u64) -> bool {
    now.saturating_sub(weather.updated_at) < MAX_AGE.as_secs()
}

/// Поточний час у секундах від 1970 року.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn cache_path(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_cache_dir().ok()?.join("weather.json"))
}

fn read_cache(app: &AppHandle) -> Option<Weather> {
    let text = std::fs::read_to_string(cache_path(app)?).ok()?;
    serde_json::from_str(&text).ok()
}

/// Помилку запису ігноруємо: погоду однаково покажемо, кеш — лише зручність.
fn write_cache(app: &AppHandle, weather: &Weather) {
    let Some(path) = cache_path(app) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string(weather) {
        let _ = std::fs::write(path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Скорочена справжня відповідь Open-Meteo.
    const SAMPLE: &str = r#"{
        "latitude": 50.45, "longitude": 30.52, "timezone": "Europe/Kyiv",
        "current": {
            "time": "2026-11-13T08:30", "interval": 900,
            "temperature_2m": 7.3, "apparent_temperature": 4.1,
            "relative_humidity_2m": 81, "weather_code": 3, "is_day": 1,
            "wind_speed_10m": 4.2, "wind_direction_10m": 225
        },
        "daily": {
            "time": ["2026-11-13"],
            "sunrise": ["2026-11-13T07:02"],
            "sunset": ["2026-11-13T16:14"]
        }
    }"#;

    #[test]
    fn parses_forecast() {
        let response: ForecastResponse = serde_json::from_str(SAMPLE).unwrap();
        let weather = to_weather(response, "Київ", 100);
        assert_eq!(weather.city, "Київ");
        assert_eq!(weather.temperature, 7.3);
        assert_eq!(weather.apparent_temperature, 4.1);
        assert_eq!(weather.humidity, 81.0);
        assert_eq!(weather.weather_code, 3);
        assert!(weather.is_day);
        assert_eq!(weather.wind_speed, 4.2);
        assert_eq!(weather.wind_direction, 225.0);
        assert_eq!(weather.sunrise, "07:02");
        assert_eq!(weather.sunset, "16:14");
        assert_eq!(weather.updated_at, 100);
    }

    #[test]
    fn empty_daily_shows_dash() {
        let json = SAMPLE
            .replace(r#"["2026-11-13T07:02"]"#, "[]")
            .replace(r#"["2026-11-13T16:14"]"#, "[]");
        let response: ForecastResponse = serde_json::from_str(&json).unwrap();
        let weather = to_weather(response, "Київ", 0);
        assert_eq!(weather.sunrise, "—");
        assert_eq!(weather.sunset, "—");
    }

    #[test]
    fn parses_geocoding_without_results() {
        let response: GeocodingResponse =
            serde_json::from_str(r#"{"generationtime_ms":0.5}"#).unwrap();
        assert!(response.results.is_empty());
    }

    #[test]
    fn rounds_coordinates_to_about_a_kilometer() {
        assert_eq!(round_coordinate(50.450_123), "50.45");
        assert_eq!(round_coordinate(30.523_9), "30.52");
        assert_eq!(round_coordinate(-0.127_6), "-0.13");
    }

    #[test]
    fn clock_time_takes_hours_and_minutes() {
        assert_eq!(clock_time("2026-11-13T07:02"), "07:02");
        assert_eq!(clock_time("07:02"), "07:02");
    }

    #[test]
    fn weather_older_than_three_hours_is_stale() {
        let response: ForecastResponse = serde_json::from_str(SAMPLE).unwrap();
        let weather = to_weather(response, "Київ", 1_000);
        assert!(is_fresh(&weather, 1_000 + 3 * 3600 - 1));
        assert!(!is_fresh(&weather, 1_000 + 3 * 3600));
    }

    /// Справжній запит до Open-Meteo. `#[ignore]` — не запускається в CI і
    /// звичайному `cargo test` (потрібен інтернет); вручну:
    /// `cargo test live_open_meteo -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_open_meteo() {
        // block_on — виконати async-код і дочекатися результату (у тесті
        // немає Tauri, що робив би це за нас)
        tauri::async_runtime::block_on(async {
            let cities = search_city("Київ", "uk").await.unwrap();
            println!("знайдено: {cities:?}");
            let city = cities.first().expect("Київ має знайтися");
            let weather = fetch_forecast(city).await.unwrap();
            println!("погода: {weather:?}");
            assert_eq!(weather.sunrise.len(), 5);
        });
    }

    #[test]
    fn city_reads_from_settings_json() {
        // Так місто записуватиме вікно налаштувань (крок 24)
        let json = r#"{"name":"Львів","admin":"Львівська область","country":"Україна","latitude":49.84,"longitude":24.02}"#;
        let city: City = serde_json::from_str(json).unwrap();
        assert_eq!(city.name, "Львів");
        assert_eq!(city.latitude, 49.84);
        // Без необов'язкових полів теж читається
        let city: City =
            serde_json::from_str(r#"{"name":"X","latitude":1,"longitude":2}"#).unwrap();
        assert_eq!(city.admin, None);
    }
}
