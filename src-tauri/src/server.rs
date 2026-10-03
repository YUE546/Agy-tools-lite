//! WebUI HTTP 服务层（无 WebView 服务版）
//!
//! 单端口同时承载：
//! - 前端静态资源（`dist/`，SPA fallback 到 index.html）
//! - `/api/{command}` 管理桥：前端 `utils/request.ts` 的 COMMAND_MAPPING 一一对应
//! - `/api/events`：SSE 事件流（订阅 `event_bus`）
//! - `/health`、`/healthz`、`/api/health`：健康检查（唯一免鉴权端点）
//!
//! 鉴权：除健康检查外所有 `/api` 请求必须携带 Web UI 密码
//! （`Authorization: Bearer <password>` 或 `x-api-key` 头）。

use axum::{
    extract::{DefaultBodyLimit, Multipart, Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::Stream;
use percent_encoding::percent_decode_str;
use serde_json::{json, Value};
use std::{
    convert::Infallible,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tokio::sync::{watch, RwLock};

use crate::{commands, event_bus};

/// 请求体上限：import_db_upload 需要容纳整份 .vscdb
const MAX_BODY_SIZE: usize = 100 * 1024 * 1024;

/// 共享服务状态
#[derive(Clone)]
struct AppState {
    admin_password: Arc<RwLock<String>>,
    /// 前端静态资源目录（dist/）
    dist_path: Arc<PathBuf>,
}

/// 运行中的服务句柄
pub struct WebServer {
    shutdown_tx: watch::Sender<bool>,
}

impl WebServer {
    /// 启动 HTTP 服务（后台 tokio 任务）。返回的句柄用于优雅停止。
    pub async fn start(bind_address: &str, port: u16, dist_path: &str) -> Result<Self, String> {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let web_config = crate::web_config::load_web_config()?;
        let password = web_config
            .admin_password
            .filter(|p| !p.is_empty())
            .unwrap_or_default();
        let state = AppState {
            admin_password: Arc::new(RwLock::new(password)),
            dist_path: Arc::new(PathBuf::from(dist_path)),
        };

        let router = build_router(state, dist_path);

        let listener = tokio::net::TcpListener::bind((bind_address, port))
            .await
            .map_err(|e| format!("无法绑定 {}:{}: {}", bind_address, port, e))?;

        let mut shutdown_rx_run = shutdown_rx.clone();
        tokio::spawn(async move {
            let shutdown = async move {
                loop {
                    if shutdown_rx_run.changed().await.is_err() || *shutdown_rx_run.borrow() {
                        break;
                    }
                }
            };
            if let Err(e) = axum::serve(listener, router)
                .with_graceful_shutdown(shutdown)
                .await
            {
                tracing::error!("HTTP server error: {}", e);
            }
        });

        Ok(Self { shutdown_tx })
    }

    /// 优雅停止（关闭监听器与活动连接）。
    pub async fn stop(&self) {
        let _ = self.shutdown_tx.send(true);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// 供运行期修改密码后同步鉴权状态（当前未暴露修改端点，预留）。
#[allow(dead_code)]
pub async fn update_admin_password(state: &AppState, password: String) {
    *state.admin_password.write().await = password;
}

fn build_router(state: AppState, dist_path: &str) -> Router {
    // 需鉴权：全部命令桥 + SSE
    let authed = api_router()
        .route("/api/events", get(admin_events))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            admin_auth_middleware,
        ))
        .layer(DefaultBodyLimit::max(MAX_BODY_SIZE));

    // 免鉴权：健康检查
    let public = Router::new()
        .route("/health", get(health))
        .route("/healthz", get(health))
        .route("/api/health", get(health));

    if !PathBuf::from(dist_path).join("index.html").is_file() {
        tracing::warn!(
            "dist/ not found at {:?}; Web UI assets unavailable, /api still works.",
            dist_path
        );
    }

    public
        .merge(authed)
        .fallback(static_fallback)
        .with_state(state)
}

/// 前端静态资源（SPA：未命中路径回退 index.html）。
/// 自带实现而非 tower-http ServeDir：行为完全可控（目录穿越防护、
/// UTF-8 路径、SPA fallback），也避免云效环境对特性解析的不确定性。
async fn static_fallback(State(state): State<AppState>, request: Request) -> Response {
    let raw_path = request.uri().path().trim_start_matches('/');
    let decoded = percent_decode_str(raw_path).decode_utf8_lossy().to_string();

    // 目录穿越防护：任何路径段不允许为 ".."
    if decoded.split(['/', '\\']).any(|segment| segment == "..") {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }

    let base = state.dist_path.as_path();
    let mut file_path = base.join(&decoded);
    if decoded.is_empty() || !file_path.is_file() {
        // SPA fallback：未命中的路由统一回 index.html
        file_path = base.join("index.html");
    }

    match tokio::fs::read(&file_path).await {
        Ok(bytes) => {
            // mime_guess::Mime 没有 as_str()，直接 Display 序列化
            let mime = mime_guess::from_path(&file_path).first_or_octet_stream().to_string();
            ([(header::CONTENT_TYPE, mime)], bytes).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

/// `/api/{command}` 命令桥路由表。
/// 新增命令时同步更新前端 `src/utils/request.ts` 的 COMMAND_MAPPING
/// （`npm run check:parity` 负责防回归）。
fn api_router() -> Router<AppState> {
    Router::new()
        // 账号
        .route("/api/list_accounts", post(cmd_list_accounts))
        .route("/api/get_current_account", post(cmd_get_current_account))
        .route(
            "/api/get_account_dashboard_snapshot",
            post(cmd_get_account_dashboard_snapshot),
        )
        .route("/api/add_account", post(cmd_add_account))
        .route("/api/delete_account", post(cmd_delete_account))
        .route("/api/delete_accounts", post(cmd_delete_accounts))
        .route("/api/reorder_accounts", post(cmd_reorder_accounts))
        .route("/api/switch_account", post(cmd_switch_account))
        .route("/api/fetch_account_quota", post(cmd_fetch_account_quota))
        .route("/api/refresh_all_quotas", post(cmd_refresh_all_quotas))
        .route("/api/export_accounts", post(cmd_export_accounts))
        .route("/api/update_account_label", post(cmd_update_account_label))
        .route("/api/import_from_db", post(cmd_import_from_db))
        .route("/api/import_custom_db", post(cmd_import_custom_db))
        .route(
            "/api/import_custom_db_upload",
            post(cmd_import_custom_db_upload),
        )
        .route("/api/sync_account_from_db", post(cmd_sync_account_from_db))
        // OAuth
        .route("/api/start_oauth_login", post(cmd_start_oauth_login))
        .route("/api/complete_oauth_login", post(cmd_complete_oauth_login))
        .route("/api/prepare_oauth_url", post(cmd_prepare_oauth_url))
        .route("/api/cancel_oauth_login", post(cmd_cancel_oauth_login))
        .route("/api/submit_oauth_code", post(cmd_submit_oauth_code))
        .route("/api/list_oauth_clients", post(cmd_list_oauth_clients))
        .route(
            "/api/get_active_oauth_client",
            post(cmd_get_active_oauth_client),
        )
        .route(
            "/api/set_active_oauth_client",
            post(cmd_set_active_oauth_client),
        )
        // 配置
        .route("/api/load_config", post(cmd_load_config))
        .route("/api/save_config", post(cmd_save_config))
        // 低配额自动切换
        .route(
            "/api/get_auto_switch_config",
            post(cmd_get_auto_switch_config),
        )
        .route(
            "/api/set_auto_switch_config",
            post(cmd_set_auto_switch_config),
        )
        .route(
            "/api/get_auto_switch_status",
            post(cmd_get_auto_switch_status),
        )
        .route("/api/cancel_auto_switch", post(cmd_cancel_auto_switch))
        .route(
            "/api/check_auto_switch_now",
            post(cmd_check_auto_switch_now),
        )
        // 本地化
        .route(
            "/api/get_app_localization_status",
            post(cmd_get_app_localization_status),
        )
        .route(
            "/api/set_app_localization_enabled",
            post(cmd_set_app_localization_enabled),
        )
        .route(
            "/api/apply_app_localization",
            post(cmd_apply_app_localization),
        )
        // 杂项
        .route("/api/get_data_dir_path", post(cmd_get_data_dir_path))
        .route("/api/open_data_folder", post(cmd_open_data_folder))
        .route("/api/get_local_token_usage", post(cmd_get_local_token_usage))
        .route("/api/get_api_pricing", post(cmd_get_api_pricing))
}

async fn health() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

// ---------- 鉴权 ----------

fn extract_credential(request: &Request) -> Option<String> {
    request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").or(Some(s)))
        .map(str::to_string)
        .or_else(|| {
            request
                .headers()
                .get("x-api-key")
                .and_then(|h| h.to_str().ok())
                .map(str::to_string)
        })
}

async fn admin_auth_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // CORS 预检放行
    if request.method() == axum::http::Method::OPTIONS {
        return Ok(next.run(request).await);
    }

    let password = state.admin_password.read().await.clone();
    if password.is_empty() {
        tracing::error!("Admin auth is required but no admin_password is configured; denying request");
        return Err(StatusCode::UNAUTHORIZED);
    }

    match extract_credential(&request) {
        Some(token) if token == password => Ok(next.run(request).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

// ---------- 事件 SSE ----------

async fn admin_events() -> Sse<impl Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
    use axum::response::sse::{Event, KeepAlive};

    let rx = event_bus::subscribe();
    let stream = async_stream::stream! {
        yield Ok(Event::default().event("connected").data("{}"));
        let mut rx = rx;
        loop {
            match rx.recv().await {
                Ok((name, data)) => {
                    yield Ok(Event::default().event(name).data(data));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                    yield Ok(Event::default()
                        .event(event_bus::LAGGED_EVENT)
                        .data(json!({ "missed": missed }).to_string()));
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    )
}

// ---------- 参数提取辅助 ----------
//
// 前端沿用 Tauri invoke 的 camelCase 键名（accountId 等），统一兼容
// snake_case / camelCase 两种键。

fn arg<'a>(args: &'a Value, snake: &str) -> Option<&'a Value> {
    let camel = to_camel_case(snake);
    args.get(snake)
        .filter(|v| !v.is_null())
        .or_else(|| args.get(&camel).filter(|v| !v.is_null()))
}

fn to_camel_case(snake: &str) -> String {
    let mut out = String::with_capacity(snake.len());
    let mut upper = false;
    for ch in snake.chars() {
        if ch == '_' {
            upper = true;
        } else if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn arg_string(args: &Value, name: &str) -> Option<String> {
    arg(args, name).and_then(|v| v.as_str().map(str::to_string))
}

fn arg_string_vec(args: &Value, name: &str) -> Option<Vec<String>> {
    arg(args, name).and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
    })
}

fn arg_bool(args: &Value, name: &str) -> Option<bool> {
    arg(args, name).and_then(|v| v.as_bool())
}

/// 缺失必填参数
fn missing(name: &str) -> (StatusCode, String) {
    (
        StatusCode::BAD_REQUEST,
        format!("missing required argument: {}", name),
    )
}

type ApiResult<T> = Result<Json<T>, (StatusCode, String)>;

fn map_err(e: String) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e)
}

// ---------- 账号 handlers ----------

async fn cmd_list_accounts() -> ApiResult<Vec<crate::models::Account>> {
    commands::list_accounts().await.map(Json).map_err(map_err)
}

async fn cmd_get_current_account() -> ApiResult<Option<crate::models::Account>> {
    commands::get_current_account()
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_get_account_dashboard_snapshot(
) -> ApiResult<crate::modules::account_dashboard::DashboardSnapshot> {
    commands::get_account_dashboard_snapshot()
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_add_account(Json(args): Json<Value>) -> ApiResult<crate::models::Account> {
    let refresh_token = arg_string(&args, "refresh_token").ok_or_else(|| missing("refresh_token"))?;
    let email = arg_string(&args, "email");
    commands::add_account(email, refresh_token)
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_delete_account(Json(args): Json<Value>) -> ApiResult<()> {
    let account_id = arg_string(&args, "account_id").ok_or_else(|| missing("account_id"))?;
    commands::delete_account(account_id)
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_delete_accounts(Json(args): Json<Value>) -> ApiResult<()> {
    let account_ids =
        arg_string_vec(&args, "account_ids").ok_or_else(|| missing("account_ids"))?;
    commands::delete_accounts(account_ids)
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_reorder_accounts(Json(args): Json<Value>) -> ApiResult<()> {
    let account_ids =
        arg_string_vec(&args, "account_ids").ok_or_else(|| missing("account_ids"))?;
    commands::reorder_accounts(account_ids)
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_switch_account(Json(args): Json<Value>) -> ApiResult<()> {
    let account_id = arg_string(&args, "account_id").ok_or_else(|| missing("account_id"))?;
    let target_ide = arg_string(&args, "target_ide");
    commands::switch_account(account_id, target_ide)
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_fetch_account_quota(Json(args): Json<Value>) -> ApiResult<crate::models::QuotaData> {
    let account_id = arg_string(&args, "account_id").ok_or_else(|| missing("account_id"))?;
    commands::fetch_account_quota(account_id)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn cmd_refresh_all_quotas() -> ApiResult<commands::RefreshStats> {
    commands::refresh_all_quotas()
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_export_accounts(
    Json(args): Json<Value>,
) -> ApiResult<crate::models::AccountExportResponse> {
    let account_ids =
        arg_string_vec(&args, "account_ids").ok_or_else(|| missing("account_ids"))?;
    commands::export_accounts(account_ids)
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_update_account_label(Json(args): Json<Value>) -> ApiResult<()> {
    let account_id = arg_string(&args, "account_id").ok_or_else(|| missing("account_id"))?;
    let label = arg_string(&args, "label").unwrap_or_default();
    commands::update_account_label(account_id, label)
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_import_from_db(Json(args): Json<Value>) -> ApiResult<Vec<crate::models::Account>> {
    let target_ide = arg_string(&args, "target_ide");
    commands::import_from_db(target_ide)
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_import_custom_db(Json(args): Json<Value>) -> ApiResult<crate::models::Account> {
    let path = arg_string(&args, "path").ok_or_else(|| missing("path"))?;
    commands::import_custom_db(path)
        .await
        .map(Json)
        .map_err(map_err)
}

/// Web 专属：multipart 直传 .vscdb 文件
async fn cmd_import_custom_db_upload(
    mut multipart: Multipart,
) -> ApiResult<crate::models::Account> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("读取上传内容失败: {}", e)))?
    {
        if field.name() == Some("file") {
            let file_name = field
                .file_name()
                .map(str::to_string)
                .unwrap_or_else(|| "import.vscdb".to_string());
            let bytes = field
                .bytes()
                .await
                .map_err(|e| (StatusCode::BAD_REQUEST, format!("读取文件内容失败: {}", e)))?;
            return commands::import_custom_db_upload(file_name, bytes.to_vec())
                .await
                .map(Json)
                .map_err(map_err);
        }
    }
    Err((StatusCode::BAD_REQUEST, "missing file field".to_string()))
}

async fn cmd_sync_account_from_db(Json(_args): Json<Value>) -> ApiResult<Option<crate::models::Account>> {
    commands::sync_account_from_db()
        .await
        .map(Json)
        .map_err(map_err)
}

// ---------- OAuth handlers ----------

async fn cmd_start_oauth_login(Json(args): Json<Value>) -> ApiResult<crate::models::Account> {
    let oauth_client_key = arg_string(&args, "oauth_client_key");
    commands::start_oauth_login(oauth_client_key)
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_complete_oauth_login() -> ApiResult<crate::models::Account> {
    commands::complete_oauth_login()
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_prepare_oauth_url(Json(args): Json<Value>) -> ApiResult<String> {
    let oauth_client_key = arg_string(&args, "oauth_client_key");
    commands::prepare_oauth_url(oauth_client_key)
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_cancel_oauth_login() -> ApiResult<()> {
    commands::cancel_oauth_login().await.map(Json).map_err(map_err)
}

async fn cmd_submit_oauth_code(Json(args): Json<Value>) -> ApiResult<()> {
    let code = arg_string(&args, "code").ok_or_else(|| missing("code"))?;
    let state = arg_string(&args, "state");
    commands::submit_oauth_code(code, state)
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_list_oauth_clients(
) -> ApiResult<Vec<crate::modules::oauth::OAuthClientDescriptor>> {
    commands::list_oauth_clients()
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_get_active_oauth_client() -> ApiResult<String> {
    commands::get_active_oauth_client()
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_set_active_oauth_client(Json(args): Json<Value>) -> ApiResult<()> {
    let client_key = arg_string(&args, "client_key").ok_or_else(|| missing("client_key"))?;
    commands::set_active_oauth_client(client_key)
        .await
        .map(Json)
        .map_err(map_err)
}

// ---------- 配置 handlers ----------

async fn cmd_load_config() -> ApiResult<crate::models::AppConfig> {
    commands::load_config().await.map(Json).map_err(map_err)
}

async fn cmd_save_config(Json(args): Json<Value>) -> ApiResult<()> {
    let config_value = arg(&args, "config").cloned().ok_or_else(|| missing("config"))?;
    let config: crate::models::AppConfig = serde_json::from_value(config_value)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid config: {}", e)))?;
    commands::save_config(config).await.map(Json).map_err(map_err)
}

// ---------- 自动切换 handlers ----------

async fn cmd_get_auto_switch_config() -> ApiResult<crate::modules::auto_switch::Config> {
    crate::modules::auto_switch::get_auto_switch_config()
        .map(Json)
        .map_err(map_err)
}

async fn cmd_set_auto_switch_config(
    Json(args): Json<Value>,
) -> ApiResult<crate::modules::auto_switch::Config> {
    let config_value = arg(&args, "config").cloned().ok_or_else(|| missing("config"))?;
    let config: crate::modules::auto_switch::Config = serde_json::from_value(config_value)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid config: {}", e)))?;
    crate::modules::auto_switch::set_auto_switch_config(config)
        .map(Json)
        .map_err(map_err)
}

async fn cmd_get_auto_switch_status() -> ApiResult<crate::modules::auto_switch::Status> {
    crate::modules::auto_switch::get_auto_switch_status()
        .map(Json)
        .map_err(map_err)
}

async fn cmd_cancel_auto_switch(Json(args): Json<Value>) -> ApiResult<crate::modules::auto_switch::Status> {
    let pending_id = arg_string(&args, "pending_id").ok_or_else(|| missing("pending_id"))?;
    crate::modules::auto_switch::cancel_auto_switch(pending_id)
        .map(Json)
        .map_err(map_err)
}

async fn cmd_check_auto_switch_now() -> ApiResult<crate::modules::auto_switch::Status> {
    crate::modules::auto_switch::check_auto_switch_now()
        .await
        .map(Json)
        .map_err(map_err)
}

// ---------- 本地化 handlers ----------

async fn cmd_get_app_localization_status(
) -> ApiResult<crate::modules::app_localization::LocalizationStatus> {
    crate::modules::app_localization::get_app_localization_status()
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_set_app_localization_enabled(
    Json(args): Json<Value>,
) -> ApiResult<crate::modules::app_localization::LocalizationStatus> {
    let enabled = arg_bool(&args, "enabled").ok_or_else(|| missing("enabled"))?;
    crate::modules::app_localization::set_app_localization_enabled(enabled)
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_apply_app_localization(
    Json(args): Json<Value>,
) -> ApiResult<crate::modules::app_localization::LocalizationStatus> {
    let launch = arg_bool(&args, "launch").unwrap_or(false);
    crate::modules::app_localization::apply_app_localization(launch)
        .await
        .map(Json)
        .map_err(map_err)
}

// ---------- 杂项 handlers ----------

async fn cmd_get_data_dir_path() -> ApiResult<String> {
    commands::get_data_dir_path().await.map(Json).map_err(map_err)
}

async fn cmd_open_data_folder() -> ApiResult<()> {
    commands::open_data_folder()
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_get_local_token_usage(
) -> ApiResult<crate::modules::native_token_stats::LocalTokenUsageSummary> {
    commands::get_local_token_usage()
        .await
        .map(Json)
        .map_err(map_err)
}

async fn cmd_get_api_pricing(
) -> ApiResult<crate::modules::api_pricing::ApiPricingSnapshot> {
    commands::get_api_pricing()
        .await
        .map(Json)
        .map_err(map_err)
}

// ---------- 测试 ----------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camel_case_conversion() {
        assert_eq!(to_camel_case("account_ids"), "accountIds");
        assert_eq!(to_camel_case("refresh_token"), "refreshToken");
        assert_eq!(to_camel_case("path"), "path");
    }

    #[test]
    fn arg_accepts_snake_and_camel_keys() {
        let snake = json!({"account_ids": ["a"]});
        let camel = json!({"accountIds": ["a"]});
        assert_eq!(arg_string_vec(&snake, "account_ids"), Some(vec!["a".to_string()]));
        assert_eq!(arg_string_vec(&camel, "account_ids"), Some(vec!["a".to_string()]));
        assert_eq!(arg_string_vec(&json!({}), "account_ids"), None);
    }
}
