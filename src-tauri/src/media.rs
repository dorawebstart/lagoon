// media.rs — що зараз грає у Windows (Spotify, YouTube у браузері, AIMP…).
//
// Джерело: Global System Media Transport Controls (GSMTC) — той самий API,
// з якого бере дані медіапанель Windows (віконце з кнопками над регулятором
// гучності). Будь-яка програма, що інтегрується з цією панеллю, видна і нам.
//
// Як влаштовано:
//   Windows викликає наші обробники подій (змінилася пісня, пауза, перемотка)
//   у своїх службових потоках. Обробники нічого не рахують — лише кидають
//   «сигнал» у канал (mpsc). Один наш потік читає ці сигнали, збирає стан і
//   надсилає його у фронтенд подією "media:update".

use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as SessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
};
use windows::Storage::Streams::{DataReader, IRandomAccessStreamReference};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::core::{Interface, Result as WinResult, RuntimeType};
use windows_future::IAsyncOperation;

/// Подія, яку слухає фронтенд.
pub const UPDATE_EVENT: &str = "media:update";

/// Різниця між «епохою» Windows (1601-01-01) і Unix (1970-01-01) у мілісекундах.
const WINDOWS_TO_UNIX_EPOCH_MS: i64 = 11_644_473_600_000;

/// Стан плеєра, який отримує фронтенд (у JSON поля стануть camelCase:
/// `isPlaying`, `positionMs`… — завдяки `rename_all`).
#[derive(Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaState {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Яка програма грає, наприклад "Spotify.exe" або "Chrome".
    pub app_id: String,
    pub is_playing: bool,
    /// Позиція і тривалість у мілісекундах.
    pub position_ms: i64,
    pub duration_ms: i64,
    /// Коли позицію було виміряно (Unix-час у мс). Фронтенд сам
    /// «докручує» прогрес від цього моменту, тож опитувати нічого не треба.
    pub updated_at_ms: i64,
    /// Обкладинка як data-URL ("data:image/png;base64,...") або None.
    pub thumbnail: Option<String>,
}

/// Останній надісланий стан. None = нічого не грає.
/// Зберігаємо, щоб фронтенд міг запитати його одразу після завантаження.
#[derive(Default)]
pub struct MediaStore(Mutex<Option<MediaState>>);

/// Команда для JS: `invoke("media_current")` — поточний стан (або null).
#[tauri::command]
pub fn media_current(store: State<MediaStore>) -> Option<MediaState> {
    // clone() — віддаємо копію, а оригінал лишається в сховищі.
    store.0.lock().unwrap().clone()
}

/// Дія з кнопок плеєра. З JS приходить рядком: "playPause" | "next" | "previous" —
/// serde сам перетворює його на варіант enum (завдяки `rename_all`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaAction {
    PlayPause,
    Next,
    Previous,
}

/// Команда для JS: `invoke("media_control", { action: "playPause" })`.
///
/// `async fn` — асинхронна функція: Tauri запускає її у фоновому пулі потоків,
/// тож інтерфейс не «зависає», поки Windows виконує дію.
/// Усередині `.await` означає «почекай результат, але не блокуй потік» —
/// так само, як `await` у JavaScript.
///
/// `Result<(), String>`: у разі помилки JS отримає відхилений Promise з текстом.
#[tauri::command]
pub async fn media_control(action: MediaAction) -> Result<(), String> {
    // map_err перетворює помилку Windows на String, а `?` одразу повертає її.
    let manager = SessionManager::RequestAsync()
        .map_err(|e| e.to_string())?
        .await
        .map_err(|e| e.to_string())?;
    let session = manager
        .GetCurrentSession()
        .map_err(|_| "нічого не грає".to_string())?;

    // match повертає значення: асинхронну операцію для потрібної дії.
    let operation = match action {
        MediaAction::PlayPause => session.TryTogglePlayPauseAsync(),
        MediaAction::Next => session.TrySkipNextAsync(),
        MediaAction::Previous => session.TrySkipPreviousAsync(),
    }
    .map_err(|e| e.to_string())?;

    // Windows повертає true/false — чи програма-плеєр прийняла команду.
    let accepted = operation.await.map_err(|e| e.to_string())?;
    if accepted {
        Ok(())
    } else {
        Err("плеєр не підтримує цю дію".to_string())
    }
}

/// Сигнали, які обробники подій Windows кидають у канал.
enum Signal {
    /// Змінилася активна сесія (запустили/закрили плеєр).
    SessionChanged,
    /// Змінилося щось усередині сесії: пісня, пауза, позиція.
    Refresh,
}

/// Запускає фоновий потік, що стежить за медіа.
pub fn start(app: AppHandle) {
    thread::spawn(move || {
        // Якщо щось не вдалося на старті (наприклад, стара Windows без GSMTC),
        // просто пишемо в лог — острів працює далі без плеєра.
        if let Err(error) = watch(&app) {
            eprintln!("media: {error}");
        }
    });
}

/// Головний цикл потоку медіа. Повертає помилку лише якщо не вдалося стартувати.
fn watch(app: &AppHandle) -> WinResult<()> {
    // WinRT-об'єкти вимагають ініціалізації в кожному потоці.
    // `unsafe` — блок, де ми обіцяємо компілятору, що виклик безпечний:
    // Rust не може сам перевірити функції Windows API.
    unsafe { RoInitialize(RO_INIT_MULTITHREADED)? };

    // RequestAsync — асинхронна операція Windows. join() чекає (блокує потік),
    // поки вона завершиться. Для окремого фонового потоку це нормально.
    let manager = SessionManager::RequestAsync()?.join()?;

    // Канал: tx (transmitter) — відправник, rx (receiver) — отримувач.
    // Відправників можна клонувати і роздати обробникам, отримувач — один.
    let (tx, rx): (Sender<Signal>, Receiver<Signal>) = mpsc::channel();

    let session_tx = tx.clone();
    manager.CurrentSessionChanged(&TypedEventHandler::new(move |_, _| {
        // send може не вдатися, лише якщо отримувача вже немає — тоді байдуже.
        let _ = session_tx.send(Signal::SessionChanged);
        Ok(())
    }))?;

    // Поточна сесія і «квитки» (tokens) її підписок — щоб потім відписатися.
    let mut subscription: Option<(Session, [i64; 3])> = None;
    let mut cached_cover: Option<CachedCover> = None;

    // Перший прохід — одразу, не чекаючи подій.
    tx.send(Signal::SessionChanged).ok();

    // `for signal in rx` — цикл чекає наступного сигналу і завершиться,
    // лише якщо всі відправники зникнуть (у нас — ніколи).
    for signal in rx.iter() {
        // Подій часто приходить кілька поспіль (пісня + позиція + статус).
        // Невелика пауза і «вичерпування» каналу збирають їх в одне оновлення.
        thread::sleep(Duration::from_millis(80));
        let mut session_changed = matches!(signal, Signal::SessionChanged);
        while let Ok(extra) = rx.try_recv() {
            session_changed |= matches!(extra, Signal::SessionChanged);
        }

        if session_changed {
            // Відписуємося від старої сесії і підписуємося на нову.
            if let Some((old, tokens)) = subscription.take() {
                unsubscribe(&old, tokens);
            }
            subscription = match manager.GetCurrentSession() {
                Ok(session) => subscribe(&session, &tx)
                    .ok()
                    .map(|tokens| (session, tokens)),
                Err(_) => None, // нічого не грає
            };
        }

        // Збираємо стан. Помилка читання = вважаємо, що нічого не грає.
        let state = match &subscription {
            Some((session, _)) => read_state(session, &mut cached_cover).ok(),
            None => None,
        };
        publish(app, state);
    }

    Ok(())
}

/// Підписується на події сесії. Повертає три «квитки» для відписки.
fn subscribe(session: &Session, tx: &Sender<Signal>) -> WinResult<[i64; 3]> {
    Ok([
        session.MediaPropertiesChanged(&refresh_handler(tx))?,
        session.PlaybackInfoChanged(&refresh_handler(tx))?,
        session.TimelinePropertiesChanged(&refresh_handler(tx))?,
    ])
}

/// Створює обробник події, який лише кидає `Signal::Refresh` у канал.
///
/// `<Args: RuntimeType + 'static>` — узагальнена (generic) функція: у трьох подій
/// різні типи аргументів, і компілятор створить окрему версію функції для
/// кожного типу. `RuntimeType + 'static` — обмеження (trait bound): «будь-який
/// тип WinRT, що не містить тимчасових посилань».
fn refresh_handler<Args: RuntimeType + 'static>(
    tx: &Sender<Signal>,
) -> TypedEventHandler<Session, Args> {
    // Кожному обробнику — своя копія відправника (move забирає її в замикання).
    let tx = tx.clone();
    TypedEventHandler::new(move |_, _| {
        let _ = tx.send(Signal::Refresh);
        Ok(())
    })
}

/// Відписується від подій сесії. Помилки ігноруємо: сесія могла вже зникнути.
fn unsubscribe(session: &Session, tokens: [i64; 3]) {
    let _ = session.RemoveMediaPropertiesChanged(tokens[0]);
    let _ = session.RemovePlaybackInfoChanged(tokens[1]);
    let _ = session.RemoveTimelinePropertiesChanged(tokens[2]);
}

/// Обкладинка, вже перетворена на data-URL, і пісня, якій вона належить.
/// Обкладинку читаємо лише коли змінилася пісня, а не на кожну паузу.
struct CachedCover {
    key: String,
    data_url: Option<String>,
}

/// Читає повний стан сесії.
fn read_state(session: &Session, cache: &mut Option<CachedCover>) -> WinResult<MediaState> {
    let props = session.TryGetMediaPropertiesAsync()?.join()?;
    let playback = session.GetPlaybackInfo()?;
    let timeline = session.GetTimelineProperties()?;

    // HSTRING — рядок Windows; to_string() перетворює його на звичайний String.
    let title = props.Title()?.to_string();
    let artist = props.Artist()?.to_string();
    let album = props.AlbumTitle()?.to_string();

    // Обкладинка: беремо з кешу, якщо пісня та сама.
    let key = format!("{title}\u{1f}{artist}\u{1f}{album}");
    let thumbnail = match cache {
        Some(cached) if cached.key == key => cached.data_url.clone(),
        _ => {
            // Якщо обкладинки немає або її не вдалося прочитати — None.
            let data_url = props.Thumbnail().ok().and_then(|r| read_cover(&r).ok());
            *cache = Some(CachedCover {
                key,
                data_url: data_url.clone(),
            });
            data_url
        }
    };

    // TimeSpan і DateTime у Windows рахуються в «тиках» по 100 нс.
    // 10 000 тиків = 1 мілісекунда.
    let position_ms = timeline.Position()?.Duration / 10_000;
    let duration_ms = timeline.EndTime()?.Duration / 10_000;
    let updated_at_ms =
        timeline.LastUpdatedTime()?.UniversalTime / 10_000 - WINDOWS_TO_UNIX_EPOCH_MS;

    Ok(MediaState {
        title,
        artist,
        album,
        app_id: session.SourceAppUserModelId()?.to_string(),
        is_playing: playback.PlaybackStatus()? == PlaybackStatus::Playing,
        position_ms,
        duration_ms,
        updated_at_ms,
        thumbnail,
    })
}

/// Читає картинку обкладинки і повертає data-URL.
fn read_cover(reference: &IRandomAccessStreamReference) -> WinResult<String> {
    let stream = reference.OpenReadAsync()?.join()?;
    let content_type = stream.ContentType()?.to_string();
    // `as u32` — розмір потоку u64, а DataReader читає u32 байт за раз
    // (обкладинка завжди значно менша за 4 ГБ).
    let size = stream.Size()? as u32;

    let reader = DataReader::CreateDataReader(&stream)?;
    // LoadAsync повертає спеціальний тип; cast() перетворює його на звичайну
    // асинхронну операцію, яку вміємо дочекатися через join().
    let load: IAsyncOperation<u32> = reader.LoadAsync(size)?.cast()?;
    let loaded = load.join()?;

    // vec![0; n] — вектор (динамічний масив) з n нулів, у нього й читаємо байти.
    let mut bytes = vec![0u8; loaded as usize];
    reader.ReadBytes(&mut bytes)?;

    let mime = if content_type.is_empty() {
        "image/png".to_string()
    } else {
        content_type
    };
    Ok(format!("data:{mime};base64,{}", BASE64.encode(&bytes)))
}

/// Зберігає новий стан і надсилає його у фронтенд, якщо він змінився.
fn publish(app: &AppHandle, state: Option<MediaState>) {
    let store = app.state::<MediaStore>();
    let mut current = store.0.lock().unwrap();
    // Порівняння працює завдяки #[derive(PartialEq)] на MediaState.
    if *current != state {
        // Лише в debug-збірці: короткий лог у термінал `npm run tauri dev`.
        #[cfg(debug_assertions)]
        match &state {
            Some(s) => eprintln!(
                "media: {} - {} [{}] playing={} {}/{} ms cover={}",
                s.title,
                s.artist,
                s.app_id,
                s.is_playing,
                s.position_ms,
                s.duration_ms,
                s.thumbnail.is_some()
            ),
            None => eprintln!("media: нічого не грає"),
        }

        *current = state.clone();
        let _ = app.emit(UPDATE_EVENT, state);
    }
}
