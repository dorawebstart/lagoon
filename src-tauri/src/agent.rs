// agent.rs — локальний HTTP-сервер, на який Claude Code надсилає свої hooks.
//
// Claude Code вміє викликати hooks типу "http": на кожну подію (почав працювати,
// запускає інструмент, просить дозволу, закінчив) він робить POST із JSON на
// нашу адресу http://127.0.0.1:47823/hook. Ми показуємо це на острові.
//
// Для запиту дозволу (подія PermissionRequest) ми НЕ відповідаємо одразу:
// тримаємо запит відкритим, поки користувач не натисне на острові
// «Дозволити» / «Відхилити» (або не мине таймаут), і лише тоді відповідаємо.
//
// Документація: https://code.claude.com/docs/en/hooks

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::State as AxumState;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_store::StoreExt;
use tokio::sync::oneshot;

use crate::settings;

/// Порт за замовчуванням (PLAN.md, 4.3).
pub const DEFAULT_PORT: u16 = 47823;

/// Заголовок, у якому Claude Code передає наш токен.
pub const TOKEN_HEADER: &str = "x-lagoon-token";

/// Скільки чекаємо рішення користувача. Має бути МЕНШЕ за `timeout`
/// у налаштуваннях hook (там радимо 60 с), інакше Claude Code обірве запит сам.
const PERMISSION_TIMEOUT: Duration = Duration::from_secs(55);

/// Події для фронтенду.
pub const EVENT: &str = "agent:event";
pub const PERMISSION_EVENT: &str = "agent:permission";
pub const PERMISSION_CLOSED_EVENT: &str = "agent:permission-closed";

/// Рішення користувача щодо запиту дозволу.
/// З JS приходить рядком: "allow" | "deny" | "ask".
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Decision {
    Allow,
    Deny,
    /// «Вирішу в терміналі»: Claude Code покаже звичайний запит у терміналі.
    Ask,
}

/// Подія для фронтенду: що зараз робить Claude Code.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEvent {
    /// Назва події hook: "UserPromptSubmit", "PreToolUse", "Stop"...
    pub kind: String,
    pub session_id: String,
    /// Назва теки проєкту (остання частина cwd), щоб розрізняти сесії.
    pub project: String,
    /// Інструмент, наприклад "Bash" або "Edit" (якщо є).
    pub tool: Option<String>,
    /// Коротко, що саме: команда, шлях до файлу, текст сповіщення...
    pub detail: Option<String>,
}

/// Запит дозволу для фронтенду.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionAsk {
    pub id: String,
    pub session_id: String,
    pub project: String,
    pub tool: String,
    pub detail: String,
}

/// Спільний стан сервера.
///
/// Його одночасно використовують:
///  - обробники HTTP-запитів (кожен запит — окрема асинхронна задача);
///  - команда `agent_respond`, яку викликає фронтенд.
///
/// Тому стан загорнуто в `Arc` (Atomically Reference Counted) — «спільне
/// володіння»: кілька частин програми тримають посилання на ті самі дані, і дані
/// живуть, поки є хоч одне посилання. А змінну частину (`pending`) захищає `Mutex`.
pub struct AgentServer {
    app: AppHandle,
    token: String,
    port: u16,
    /// Запити дозволу, що чекають відповіді: id → «канал», куди покласти рішення.
    pending: Mutex<HashMap<String, oneshot::Sender<Decision>>>,
}

/// Те, що Tauri зберігає в `.manage(...)`, щоб команди мали доступ до сервера.
pub struct AgentHandle(pub Arc<AgentServer>);

/// Інформація для вікна налаштувань: порт і токен (крок 9 — «Скопіювати конфіг»).
#[derive(Serialize)]
pub struct AgentInfo {
    pub port: u16,
    pub token: String,
}

/// Команда для JS: `invoke("agent_info")`.
#[tauri::command]
pub fn agent_info(agent: State<AgentHandle>) -> AgentInfo {
    AgentInfo {
        port: agent.0.port,
        token: agent.0.token.clone(),
    }
}

/// Команда для JS: `invoke("agent_respond", { id, decision: "allow" })`.
/// Повертає false, якщо запит уже неактуальний (таймаут або Claude Code його скасував).
#[tauri::command]
pub fn agent_respond(id: String, decision: Decision, agent: State<AgentHandle>) -> bool {
    // remove() дістає відправника з таблиці (і забирає його у власність).
    let sender = agent.0.pending.lock().unwrap().remove(&id);
    match sender {
        // send() передає рішення обробнику HTTP-запиту, що чекає на нього.
        Some(sender) => sender.send(decision).is_ok(),
        None => false,
    }
}

/// Створює стан сервера і запускає його у фоні. Повертає handle для `.manage()`.
pub fn start(app: &AppHandle) -> AgentHandle {
    let server = Arc::new(AgentServer {
        app: app.clone(),
        token: load_or_create_token(app),
        port: DEFAULT_PORT,
        pending: Mutex::new(HashMap::new()),
    });

    // Arc::clone не копіює дані — лише збільшує лічильник посилань.
    let for_server = Arc::clone(&server);

    // async_runtime::spawn запускає асинхронну задачу в рантаймі tokio, який
    // уже працює всередині Tauri. `async move { ... }` — асинхронний блок,
    // що забирає for_server собі.
    tauri::async_runtime::spawn(async move {
        if let Err(error) = serve(for_server).await {
            // Найчастіше: порт уже зайнятий (наприклад, запущено дві копії острова).
            eprintln!("agent: сервер не запустився: {error}");
        }
    });

    AgentHandle(server)
}

/// Слухає 127.0.0.1:порт і обробляє запити. Працює, поки працює застосунок.
async fn serve(server: Arc<AgentServer>) -> std::io::Result<()> {
    let port = server.port;

    // Маршрути: як routes/web.php у Laravel.
    let router = Router::new()
        .route("/health", get(|| async { "lagoon" }))
        .route("/hook", post(handle_hook))
        // with_state — спільний стан, який axum передасть кожному обробнику.
        .with_state(server);

    // Тільки 127.0.0.1: до сервера можна достукатися лише з цього комп'ютера.
    // `.await` — чекаємо, поки ОС відкриє порт, не блокуючи потік.
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    #[cfg(debug_assertions)]
    eprintln!("agent: слухаю http://127.0.0.1:{port}");
    axum::serve(listener, router).await
}

/// Обробник POST /hook. Сюди приходять усі події Claude Code.
///
/// Аргументи axum «витягує» із запиту сам (extractors), за їхніми типами:
/// стан, заголовки і JSON-тіло.
async fn handle_hook(
    AxumState(server): AxumState<Arc<AgentServer>>,
    headers: HeaderMap,
    Json(input): Json<Value>,
) -> Response {
    // 1. Перевіряємо токен: без нього запит міг надіслати будь-хто на цьому ПК.
    let token = headers.get(TOKEN_HEADER).and_then(|v| v.to_str().ok());
    if token != Some(server.token.as_str()) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    // 2. Загальні поля, які Claude Code надсилає з кожною подією.
    // input["поле"] повертає Value::Null, якщо поля немає, тож паніки не буде.
    let kind = text(&input["hook_event_name"]);
    let session_id = text(&input["session_id"]);
    let project = project_name(&text(&input["cwd"]));
    let tool = input["tool_name"].as_str().map(String::from);

    if kind == "PermissionRequest" {
        let ask = PermissionAsk {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            project,
            tool: tool.clone().unwrap_or_default(),
            detail: summarize(tool.as_deref(), &input["tool_input"]),
        };
        return wait_for_decision(server, ask).await;
    }

    // 3. Решта подій — лише показуємо на острові і одразу відповідаємо.
    let detail = match kind.as_str() {
        "PreToolUse" | "PostToolUse" => Some(summarize(tool.as_deref(), &input["tool_input"])),
        "Notification" => input["message"].as_str().map(String::from),
        _ => None,
    };
    let event = AgentEvent {
        kind,
        session_id,
        project,
        tool,
        detail,
    };
    #[cfg(debug_assertions)]
    eprintln!(
        "agent: {} [{}] {:?} {:?}",
        event.kind, event.project, event.tool, event.detail
    );
    let _ = server.app.emit(EVENT, event);

    // 200 з порожнім тілом = «все гаразд, нічого не змінюю».
    StatusCode::OK.into_response()
}

/// Показує запит дозволу на острові і чекає рішення користувача.
async fn wait_for_decision(server: Arc<AgentServer>, ask: PermissionAsk) -> Response {
    // oneshot — канал на одне повідомлення: tx віддаємо в таблицю pending,
    // а самі чекаємо на rx.
    let (tx, rx) = oneshot::channel();
    server.pending.lock().unwrap().insert(ask.id.clone(), tx);

    // «Сторож»: коли ця функція завершиться БУДЬ-ЯК (відповідь, таймаут або
    // Claude Code сам обірвав з'єднання), Drop прибере запит і сховає кнопки.
    let _guard = PendingGuard {
        server: Arc::clone(&server),
        id: ask.id.clone(),
    };

    #[cfg(debug_assertions)]
    eprintln!("agent: дозвіл? {} {} — {}", ask.id, ask.tool, ask.detail);
    // Острів схований (Ctrl+Alt+L) — показуємо: запит дозволу важливіший
    crate::visibility::show(&server.app);
    let _ = server.app.emit(PERMISSION_EVENT, ask);

    // timeout обгортає очікування: або рішення прийде, або мине PERMISSION_TIMEOUT.
    let decision = match tokio::time::timeout(PERMISSION_TIMEOUT, rx).await {
        Ok(Ok(decision)) => decision,
        // Err від timeout — час вийшов; Ok(Err) — відправника знищено. В обох випадках
        // нехай користувач вирішить у терміналі, як зазвичай.
        _ => Decision::Ask,
    };

    match decision {
        Decision::Allow => Json(permission_output(json!({ "behavior": "allow" }))).into_response(),
        Decision::Deny => Json(permission_output(json!({
            "behavior": "deny",
            "message": "Відхилено користувачем в острові Lagoon"
        })))
        .into_response(),
        // Порожня відповідь: рішення немає, Claude Code покаже звичайний запит.
        Decision::Ask => StatusCode::OK.into_response(),
    }
}

/// Відповідь у форматі, якого чекає Claude Code для PermissionRequest.
fn permission_output(decision: Value) -> Value {
    json!({
        "hookSpecificOutput": {
            "hookEventName": "PermissionRequest",
            "decision": decision
        }
    })
}

/// Прибирає запит із таблиці, коли обробник завершується.
///
/// Трейт `Drop` — код, який Rust викликає автоматично, коли значення
/// «помирає» (виходить з області видимості). Схоже на деструктор у PHP.
struct PendingGuard {
    server: Arc<AgentServer>,
    id: String,
}

impl Drop for PendingGuard {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        eprintln!("agent: запит {} закрито", self.id);
        self.server.pending.lock().unwrap().remove(&self.id);
        let _ = self.server.app.emit(PERMISSION_CLOSED_EVENT, &self.id);
    }
}

/// Коротко описує дію інструмента, щоб влізло в острів.
fn summarize(tool: Option<&str>, input: &Value) -> String {
    let field = match tool {
        Some("Bash") | Some("PowerShell") => "command",
        Some("Edit") | Some("Write") | Some("Read") | Some("MultiEdit") | Some("NotebookEdit") => {
            "file_path"
        }
        Some("WebFetch") => "url",
        Some("WebSearch") => "query",
        Some("Grep") | Some("Glob") => "pattern",
        _ => "",
    };

    let full = match input[field].as_str() {
        Some(value) if !field.is_empty() => value.to_string(),
        // Невідомий інструмент — показуємо JSON аргументів.
        _ if input.is_null() => String::new(),
        _ => input.to_string(),
    };
    shorten(&full, 160)
}

/// Обрізає рядок до `max` символів, додаючи «…».
/// Рахуємо саме символи (chars), а не байти: українська літера займає 2 байти,
/// і обрізання посеред літери зламало б рядок.
fn shorten(text: &str, max: usize) -> String {
    let single_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if single_line.chars().count() <= max {
        single_line
    } else {
        let cut: String = single_line.chars().take(max - 1).collect();
        format!("{cut}…")
    }
}

/// Рядок із JSON-значення або порожній рядок.
fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_string()
}

/// "C:\\git\\lagoon" → "lagoon"
fn project_name(cwd: &str) -> String {
    cwd.rsplit(['\\', '/'])
        .find(|part| !part.is_empty())
        .unwrap_or_default()
        .to_string()
}

/// Токен зберігається в settings.json. Якщо його ще немає — створюємо випадковий.
fn load_or_create_token(app: &AppHandle) -> String {
    let Ok(store) = app.store(settings::STORE_FILE) else {
        // Сховище недоступне — токен лише на цей запуск.
        return uuid::Uuid::new_v4().simple().to_string();
    };

    if let Some(token) = store
        .get("agentToken")
        .and_then(|v| v.as_str().map(String::from))
        && !token.is_empty()
    {
        return token;
    }

    let token = uuid::Uuid::new_v4().simple().to_string();
    store.set("agentToken", json!(token));
    let _ = store.save();
    token
}
