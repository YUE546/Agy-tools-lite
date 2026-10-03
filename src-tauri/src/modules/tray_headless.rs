// 无 WebView 服务版（headless）系统托盘（Windows）。
// 服务模式没有 tauri 窗口运行时可用，因此直接使用 tray-icon crate：
// Windows 上托盘与菜单回调依赖 Win32 消息循环，所以托盘在独立线程上创建，
// 并由该线程持续泵消息（GetMessageW）；服务主循环（tokio runtime）不受影响。
// 退出直接结束进程（状态写入均为原子落盘，无长期持有的子进程）。

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::TrayIconBuilder;

use tracing::{error, info, warn};

pub struct HeadlessTrayContext {
    /// WebUI/管理面端口（用于"打开 Web UI"）
    pub port: u16,
}

/// 在独立线程上创建托盘并泵消息。失败只记日志，不影响服务运行。
pub fn spawn_tray(ctx: HeadlessTrayContext) {
    let result = std::thread::Builder::new()
        .name("headless-tray".into())
        .spawn(move || run_tray(ctx));
    if let Err(e) = result {
        error!("Failed to spawn headless tray thread: {}", e);
    }
}

fn menu_texts() -> (&'static str, &'static str, &'static str) {
    // (打开 Web UI, 退出, tooltip)
    let zh = crate::modules::load_app_config()
        .map(|c| c.language.starts_with("zh"))
        .unwrap_or(true);
    if zh {
        ("打开 Web UI", "退出", "Antigravity Tools Lite 服务运行中")
    } else {
        ("Open Web UI", "Quit", "Antigravity Tools Lite service running")
    }
}

fn run_tray(ctx: HeadlessTrayContext) {
    let (open_text, quit_text, tooltip) = menu_texts();

    let icon = match load_icon() {
        Ok(icon) => icon,
        Err(e) => {
            error!("Headless tray: failed to load icon: {}", e);
            return;
        }
    };

    // 菜单：打开 Web UI / 退出
    let menu = Menu::new();
    let open_item = MenuItem::with_id("open_webui", open_text, true, None);
    let quit_item = MenuItem::with_id("quit", quit_text, true, None);
    if let Err(e) = menu.append_items(&[&open_item, &PredefinedMenuItem::separator(), &quit_item]) {
        error!("Headless tray: failed to build menu: {}", e);
        return;
    }

    let tray = TrayIconBuilder::new()
        .with_id("headless")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(true)
        .with_icon(icon)
        .with_tooltip(tooltip)
        .build();

    if let Err(e) = tray {
        error!("Headless tray: failed to create tray icon: {}", e);
        return;
    }
    info!("Headless system tray created.");

    // 全局菜单事件处理器（Windows 上经消息循环在托盘线程回调）
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| match event.id().as_ref() {
        "open_webui" => {
            let url = format!("http://localhost:{}", ctx.port);
            crate::open_in_system_browser(&url);
        }
        "quit" => {
            info!("Headless tray: quit requested, exiting...");
            std::process::exit(0);
        }
        _ => {}
    }));

    pump_messages_forever();
}

fn load_icon() -> Result<tray_icon::Icon, String> {
    let icon_bytes: &[u8] = include_bytes!("../../icons/icon.png");
    let img = image::load_from_memory(icon_bytes).map_err(|e| e.to_string())?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    tray_icon::Icon::from_rgba(rgba.into_raw(), width, height).map_err(|e| e.to_string())
}

/// 托盘线程消息泵：GetMessageW 返回 0（WM_QUIT）或 -1（错误）时结束循环。
/// 服务进程退出走 std::process::exit，因此正常情况下此循环与进程同生命周期。
fn pump_messages_forever() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, TranslateMessage, MSG,
    };

    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        loop {
            let ret = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
            if ret <= 0 {
                if ret < 0 {
                    warn!("Headless tray: GetMessageW failed; tray thread exiting");
                }
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
