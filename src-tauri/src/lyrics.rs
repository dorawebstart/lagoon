// lyrics.rs — текст пісні з LRCLIB (https://lrclib.net).
//
// LRCLIB — безкоштовна відкрита база текстів без API-ключа. Синхронізований
// текст приходить у форматі LRC: кожен рядок має мітку часу, наприклад
//   [00:12.34] Перший рядок
//   [00:15.80] Другий рядок
// Ми розбираємо його на список (час у мс, текст), а фронтенд за позицією
// пісні підсвічує поточний рядок.
//
// Знайдені тексти кешуємо на диску (папка кешу застосунку → lyrics/*.json),
// щоб не питати сервер щоразу, коли пісня грає знову.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const API: &str = "https://lrclib.net/api";

/// Один рядок синхронізованого тексту. У JSON: `{ "timeMs": 12340, "text": "…" }`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub time_ms: i64,
    pub text: String,
}

/// Текст пісні, який отримує фронтенд (і який лежить у кеші).
#[derive(Clone, Serialize, Deserialize)]
pub struct Lyrics {
    /// Рядки з мітками часу; порожній, якщо синхронізованого тексту немає.
    pub synced: Vec<LyricLine>,
    /// Звичайний текст без міток (поки не показуємо, але зберігаємо на майбутнє).
    pub plain: Option<String>,
}

/// Відповідь LRCLIB (лише потрібні нам поля; решту serde проігнорує).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibTrack {
    /// Тривалість у секундах.
    duration: Option<f64>,
    instrumental: Option<bool>,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

/// Команда для JS:
/// `invoke("lyrics_get", { artist, title, album, durationMs })` → `Lyrics | null`.
///
/// `async` + `.await` — поки чекаємо відповідь сервера, потік не блокується
/// (так само як `await fetch()` у JavaScript).
#[tauri::command]
pub async fn lyrics_get(
    app: AppHandle,
    artist: String,
    title: String,
    album: String,
    duration_ms: i64,
) -> Result<Option<Lyrics>, String> {
    if artist.trim().is_empty() || title.trim().is_empty() {
        return Ok(None);
    }
    let duration_s = duration_ms / 1000;

    // 1. Кеш: якщо вже шукали цю пісню — віддаємо з диска.
    let cache_file = cache_path(&app, &artist, &title, duration_s);
    if let Some(cached) = cache_file.as_deref().and_then(read_cache) {
        return Ok(Some(cached));
    }

    // 2. Сервер.
    let Some(track) = fetch(&artist, &title, &album, duration_s)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    if track.instrumental == Some(true) {
        return Ok(None);
    }

    let synced = track
        .synced_lyrics
        .as_deref()
        .map(parse_lrc)
        .unwrap_or_default();
    let plain = track.plain_lyrics.filter(|text| !text.trim().is_empty());
    if synced.is_empty() && plain.is_none() {
        return Ok(None);
    }

    let lyrics = Lyrics { synced, plain };
    // 3. Зберігаємо в кеш. Помилку запису ігноруємо: текст усе одно покажемо.
    if let Some(path) = cache_file {
        write_cache(&path, &lyrics);
    }
    Ok(Some(lyrics))
}

/// Шукає пісню на LRCLIB. Спершу точний запит `/get` (назва + виконавець +
/// альбом + тривалість), якщо не знайдено — пошук `/search` за назвою і виконавцем.
async fn fetch(
    artist: &str,
    title: &str,
    album: &str,
    duration_s: i64,
) -> reqwest::Result<Option<LrclibTrack>> {
    // LRCLIB просить, щоб застосунок представлявся в User-Agent.
    // env! підставляє значення з Cargo.toml під час компіляції.
    let user_agent = format!(
        "Lagoon/{} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_REPOSITORY")
    );
    let client = reqwest::Client::builder()
        .user_agent(user_agent)
        .timeout(Duration::from_secs(10))
        .build()?;

    let duration = duration_s.to_string();
    let response = client
        .get(format!("{API}/get"))
        // query() сам екранує спецсимволи (пробіли, &, кирилицю) в URL.
        .query(&[
            ("artist_name", artist),
            ("track_name", title),
            ("album_name", album),
            ("duration", duration.as_str()),
        ])
        .send()
        .await?;
    if response.status().is_success() {
        // json::<T>() розбирає тіло відповіді в нашу структуру через serde.
        return Ok(Some(response.json::<LrclibTrack>().await?));
    }

    // Точного збігу немає (часто через альбом чи тривалість) — шукаємо ширше.
    let results: Vec<LrclibTrack> = client
        .get(format!("{API}/search"))
        .query(&[("artist_name", artist), ("track_name", title)])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(pick_best(results, duration_s))
}

/// З результатів пошуку обирає пісню із синхронізованим текстом і
/// найближчою тривалістю (не далі 5 с), інакше — першу з будь-яким текстом.
fn pick_best(results: Vec<LrclibTrack>, duration_s: i64) -> Option<LrclibTrack> {
    let close = |track: &LrclibTrack| {
        track
            .duration
            .is_none_or(|d| (d - duration_s as f64).abs() <= 5.0)
    };
    // position() — індекс першого елемента, що задовольняє умову.
    let index = results
        .iter()
        .position(|t| t.synced_lyrics.is_some() && close(t))
        .or_else(|| {
            results
                .iter()
                .position(|t| t.plain_lyrics.is_some() && close(t))
        })?;
    // into_iter().nth() забирає елемент із вектора (без копіювання).
    results.into_iter().nth(index)
}

/// Розбирає LRC у відсортований список рядків.
///
/// Особливості формату:
///   - рядок може мати кілька міток: `[00:12.00][01:30.50]Приспів`;
///   - метадані на кшталт `[ar:Виконавець]` — не час, їх пропускаємо;
///   - порожній текст після мітки — пауза (програш), теж зберігаємо.
pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();

    for raw in text.lines() {
        // `rest` — ще не розібрана частина рядка. &str — «зріз» рядка:
        // ми не копіюємо текст, а лише зсуваємо початок.
        let mut rest = raw.trim();
        let mut times = Vec::new();

        while let Some(after_bracket) = rest.strip_prefix('[') {
            let Some(end) = after_bracket.find(']') else {
                break;
            };
            match parse_timestamp(&after_bracket[..end]) {
                Some(ms) => times.push(ms),
                None => break, // метадані, а не час
            }
            rest = &after_bracket[end + 1..];
        }

        let text = rest.trim();
        for time_ms in times {
            lines.push(LyricLine {
                time_ms,
                text: text.to_string(),
            });
        }
    }

    // Через кілька міток в одному рядку порядок може збитися — сортуємо за часом.
    lines.sort_by_key(|line| line.time_ms);
    lines
}

/// "01:02.50" → 62500 мс. Повертає None, якщо це не час (наприклад, "ar:Name").
fn parse_timestamp(tag: &str) -> Option<i64> {
    // split_once ділить рядок за першим ':' на дві частини.
    let (minutes, seconds) = tag.split_once(':')?;
    // parse::<T>() повертає Result; .ok()? — при помилці виходимо з None.
    let minutes: i64 = minutes.trim().parse().ok()?;
    let seconds: f64 = seconds.trim().parse().ok()?;
    if minutes < 0 || !(0.0..60.0).contains(&seconds) {
        return None;
    }
    Some(minutes * 60_000 + (seconds * 1000.0).round() as i64)
}

/// Шлях до файлу кешу для пісні або None, якщо папку кешу не знайдено.
fn cache_path(app: &AppHandle, artist: &str, title: &str, duration_s: i64) -> Option<PathBuf> {
    let key = format!(
        "{}\u{1f}{}\u{1f}{}",
        artist.trim().to_lowercase(),
        title.trim().to_lowercase(),
        duration_s
    );
    // Ім'я файлу — хеш ключа: так у ньому немає заборонених символів (/ : ? …).
    // {:016x} — 16 шістнадцяткових цифр із нулями попереду.
    let file = format!("{:016x}.json", fnv1a(key.as_bytes()));
    let dir = app.path().app_cache_dir().ok()?.join("lyrics");
    Some(dir.join(file))
}

fn read_cache(path: &Path) -> Option<Lyrics> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_cache(path: &Path, lyrics: &Lyrics) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string(lyrics) {
        let _ = std::fs::write(path, text);
    }
}

/// Хеш FNV-1a: простий і стабільний (той самий результат у будь-якій версії
/// Rust), тож імена файлів кешу не зміняться після оновлення.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        // wrapping_mul — множення, яке при переповненні «загортається»,
        // а не зупиняє програму (у debug-збірці звичайне * панікувало б).
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_timestamps() {
        assert_eq!(parse_timestamp("00:12.34"), Some(12_340));
        assert_eq!(parse_timestamp("01:02.5"), Some(62_500));
        assert_eq!(parse_timestamp("03:00.123"), Some(180_123));
        assert_eq!(parse_timestamp("02:05"), Some(125_000));
        assert_eq!(parse_timestamp("ar:Lil Peep"), None);
        assert_eq!(parse_timestamp("00:75.00"), None);
    }

    #[test]
    fn parses_lrc() {
        let lrc = "[ar:Someone]\n[00:05.00]Перший\n[00:01.00][00:09.50]Приспів\n[00:07.00]\n";
        let lines = parse_lrc(lrc);
        let times: Vec<i64> = lines.iter().map(|l| l.time_ms).collect();
        assert_eq!(times, vec![1_000, 5_000, 7_000, 9_500]);
        assert_eq!(lines[0].text, "Приспів");
        assert_eq!(lines[1].text, "Перший");
        assert_eq!(lines[2].text, ""); // пауза
        assert_eq!(lines[3].text, "Приспів");
    }

    #[test]
    fn fnv1a_is_stable() {
        // Відоме значення FNV-1a для порожнього рядка і для "a".
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
