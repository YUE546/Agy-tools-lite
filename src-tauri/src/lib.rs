//! Antigravity Tools Lite —— 无 WebView 服务版
//!
//! 单二进制：axum 单端口同时承载 WebUI 静态页面、`/api` 管理桥与 `/api/events`
//! SSE 事件流；Windows 下带系统托盘（独立线程 + Win32 消息泵）。
//! 不依赖 tauri / wry / WebView2，UI 由系统浏览器呈现。

pub mod cli;
mod commands;
pub mod constants;
pub mod error;
pub mod event_bus;
mod models;
mod modules;
pub mod server;
mod utils;
pub mod web_config;

use tracing::info;

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn should_enable_tray() -> bool {
    if env_flag_enabled("ANTIGRAVITY_DISABLE_TRAY") {
        info!("Tray disabled by ANTIGRAVITY_DISABLE_TRAY");
        return false;
    }
    true
}

/// Windows FFI calls to disable Efficiency Mode (EcoQoS / Power Throttling)
/// to prevent background freezes when running as a background service.
#[cfg(target_os = "windows")]
mod windows_api {
    type Bool = i32;
    type Handle = *mut std::ffi::c_void;

    #[repr(C)]
    struct ProcessPowerThrottlingState {
        version: u32,
        control_mask: u32,
        state_mask: u32,
    }

    #[link(name = "Kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> Handle;
        fn SetProcessInformation(
            h_process: Handle,
            process_information_class: u32,
            process_information: *mut std::ffi::c_void,
            process_information_size: u32,
        ) -> Bool;
    }

    pub fn disable_efficiency_mode() {
        unsafe {
            let mut state = ProcessPowerThrottlingState {
                version: 1,        // PROCESS_POWER_THROTTLING_STATE::VERSION
                control_mask: 0x1, // PROCESS_POWER_THROTTLING_CURRENT_EXECUTION_SPEED
                state_mask: 0,
            };
            let process_handle = GetCurrentProcess();
            // ProcessPowerThrottling = 4
            let res = SetProcessInformation(
                process_handle,
                4,
                &mut state as *mut _ as *mut std::ffi::c_void,
                std::mem::size_of::<ProcessPowerThrottlingState>() as u32,
            );
            if res == 0 {
                let err = std::io::Error::last_os_error();
                tracing::warn!(
                    "Failed to disable Windows Power Throttling / EcoQoS: {}",
                    err
                );
            } else {
                tracing::info!(
                    "Successfully disabled Windows Power Throttling / EcoQoS for the process."
                );
            }
        }
    }
}

/// 用系统默认浏览器打开 URL（`--open`；服务托盘"打开 Web UI"也走这里）
pub(crate) fn open_in_system_browser(url: &str) {
    let result = open_in_system_browser_impl(url);
    if let Err(e) = result {
        tracing::error!("Failed to open browser: {}", e);
    }
}

#[cfg(target_os = "windows")]
fn open_in_system_browser_impl(url: &str) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn()
        .map(|_| ())
}

#[cfg(target_os = "macos")]
fn open_in_system_browser_impl(url: &str) -> std::io::Result<()> {
    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .map(|_| ())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_in_system_browser_impl(url: &str) -> std::io::Result<()> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map(|_| ())
}

/// 服务模式主入口：WebUI + /api + 托盘 + 后台任务，阻塞至 Ctrl-C / 托盘退出。
pub fn run_service() {
    modules::logger::init_logger();

    #[cfg(target_os = "windows")]
    windows_api::disable_efficiency_mode();

    let explicit_headless = std::env::args()
        .any(|arg| arg == "--headless" || arg == "--serve");
    if explicit_headless {
        info!("Running in service mode (--headless given explicitly).");
    }

    // 浏览器拉起策略：--no-open 强制不拉；--open 强制拉；都不带时按启动方式推断——
    // 双击 exe 直启（无显式 --headless/--serve）默认拉起浏览器，脚本化运行（显式
    // --headless/--serve）默认安静后台跑。
    let open_browser = if std::env::args().any(|arg| arg == "--no-open") {
        false
    } else if std::env::args().any(|arg| arg == "--open") {
        true
    } else {
        !explicit_headless
    };

    run_headless(open_browser);
}

/// 无窗口服务模式：单 tokio runtime 托管 WebUI 静态资源 + /api 管理桥。
fn run_headless(open_browser: bool) {
    info!("Starting in HEADLESS (WebUI service) mode...");

    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    rt.block_on(async {
        // 服务配置（web_config.json + 环境变量覆盖，密码为空则生成并打印）
        let (web_config, bind_address) = match web_config::effective_service_config() {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("Failed to load web service config: {}", e);
                std::process::exit(1);
            }
        };

        info!("--------------------------------------------------");
        info!("🚀 Antigravity Tools Lite WebUI service starting...");
        info!("📍 Bind: {}:{} (WebUI + /api)", bind_address, web_config.port);
        info!(
            "🔑 Web UI Password: {}",
            match &web_config.admin_password {
                Some(pwd) if !pwd.is_empty() => {
                    let masked: String = pwd.chars().take(2).collect();
                    format!("{}**", masked)
                }
                // 双击直启无控制台，完整密码只落在数据目录的日志文件里
                _ => match modules::account::get_data_dir() {
                    Ok(dir) => format!("(generated — see {} logs)", dir.display()),
                    Err(_) => "(generated, see service log file)".to_string(),
                },
            }
        );
        info!("💡 Data directory: {:?}", modules::account::get_data_dir().ok());
        info!("--------------------------------------------------");

        // 前端静态资源目录（release 包内为 ./dist；ABV_DIST_PATH 可覆盖）
        let dist_path = std::env::var("ABV_DIST_PATH").unwrap_or_else(|_| "dist".to_string());

        // 启动 HTTP 服务
        let server = server::WebServer::start(&bind_address, web_config.port, &dist_path).await;
        let server = match server {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to start WebUI service: {}", e);
                std::process::exit(1);
            }
        };
        info!("WebUI service is running.");

        // --open：服务启动成功后再拉起浏览器
        if open_browser {
            let url = format!("http://localhost:{}", web_config.port);
            std::thread::spawn(move || open_in_system_browser(&url));
        }

        // 低配额自动切换后台循环（5s 轮询）
        modules::auto_switch::start();
        info!("Auto-switch scheduler started.");

        // 系统托盘（Windows）：独立线程创建并泵 Win32 消息。
        // ANTIGRAVITY_DISABLE_TRAY=1 可禁用；失败只记日志，不影响服务。
        #[cfg(target_os = "windows")]
        if should_enable_tray() {
            modules::tray_headless::spawn_tray(modules::tray_headless::HeadlessTrayContext {
                port: web_config.port,
            });
        }

        // Wait for Ctrl-C
        tokio::signal::ctrl_c().await.ok();
        info!("Service shutting down");
        server.stop().await;
    });
}
