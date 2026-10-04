//! macOS overview is an NSMenu, like CodexBar: AppKit owns the surface and
//! tracking; custom content uses semantic system text and small quota bars.
//! No WebView transparency, root-view replacement, or credential DTOs.
use super::{account_dashboard::{DashboardSnapshot, DashboardEntry}, menu_bar_projection as projection};
use crate::{commands, modules, models::AppConfig};
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadOnly};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSApplication, NSAlert, NSAlertFirstButtonReturn, NSBezierPath, NSButton, NSBezelStyle, NSButtonType, NSControlStateValueOn, NSCellImagePosition, NSControlSize, NSColor, NSFont, NSImage, NSImageView, NSMenu, NSMenuItem, NSTextField, NSView};
use crate::models::config::{MenuBarPreferences, MenuBarPreferencesPatch, MenuBarQuotaScope};
use tauri_plugin_opener::OpenerExt;
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use std::{cell::RefCell, sync::{Mutex, atomic::{AtomicBool, AtomicU64, Ordering}}};
use tauri::Emitter;

const WIDTH: f64 = 380.0;
const BRAND: &str = "AntiGravity tool lite";
const GITHUB: &str = "https://github.com/anglee0323/antigravity-tools-lite";
static OPEN: AtomicBool = AtomicBool::new(false);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static BUSY: AtomicBool = AtomicBool::new(false);
static REOPEN: AtomicBool = AtomicBool::new(false);
static NOTICE: Mutex<Option<String>> = Mutex::new(None);
thread_local! { static ACTIVE: RefCell<Option<Retained<NSMenu>>> = const { RefCell::new(None) }; }

#[derive(Clone)]
enum Action { Switch(String), Page(&'static str), Refresh, Cancel(String), Filter(MenuBarQuotaScope), About, Github, Quit, Noop }
struct ActionState { app: tauri::AppHandle, menu: Retained<NSMenu>, action: Action, zh: bool }
define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ActionState]
    struct MenuAction;
    unsafe impl NSObjectProtocol for MenuAction {}
    impl MenuAction {
        #[unsafe(method(perform:))]
        fn perform(&self, _sender: &AnyObject) {
            let state = self.ivars();
            let mut root = state.menu.clone();
            // All parent menus are retained during this tracking session.
            while let Some(parent) = unsafe { root.supermenu() } { root = parent; }
            root.cancelTrackingWithoutAnimation();
            let app = state.app.clone();
            let zh = state.zh;
            match &state.action {
                Action::Quit => app.exit(0),
                Action::Noop => {},
                Action::Github => { let _ = app.opener().open_url(GITHUB, None::<&str>); },
                Action::About => {
                    let alert = NSAlert::new(self.mtm());
                    alert.setMessageText(&NSString::from_str(BRAND));
                    alert.setInformativeText(&NSString::from_str(&format!("{} {}\n\nGitHub\n{GITHUB}", if zh { "版本" } else { "Version" }, app.package_info().version)));
                    alert.addButtonWithTitle(&NSString::from_str(if zh { "打开 GitHub" } else { "Open GitHub" }));
                    alert.addButtonWithTitle(&NSString::from_str(if zh { "关闭" } else { "Close" }));
                    if alert.runModal() == NSAlertFirstButtonReturn { let _ = app.opener().open_url(GITHUB, None::<&str>); }
                },
                Action::Filter(scope) => {
                    let scope = *scope;
                    tauri::async_runtime::spawn(async move {
                        let patch = MenuBarPreferencesPatch { display_scope: Some(scope), ..Default::default() };
                        if commands::set_menu_bar_preferences(app.clone(), None, Some(patch)).await.is_err() {
                            if let Ok(mut notice) = NOTICE.lock() { *notice = Some(if zh { "系列选择保存失败，请重试".into() } else { "Could not save the family selection. Retry.".into() }); }
                        }
                        let handle = app.clone();
                        let _ = handle.run_on_main_thread(move || {
                            if OPEN.load(Ordering::Acquire) { REOPEN.store(true, Ordering::Release); }
                            else { let _ = toggle(&app, None); }
                        });
                    });
                },
                Action::Page(page) => { let _ = modules::desktop::open_app_page(app, (*page).into()); },
                action => {
                    if BUSY.swap(true, Ordering::AcqRel) { return; }
                    let action = action.clone();
                    tauri::async_runtime::spawn(async move {
                        let result = match action {
                            Action::Switch(id) => commands::switch_account(app.clone(), id.clone(), None).await.map(|()| {
                                let _ = app.emit("tray://account-switched", id);
                                if zh { "账号已切换".into() } else { "Account switched".into() }
                            }),
                            Action::Refresh => commands::refresh_all_quotas(app.clone()).await.map(|stats| {
                                if stats.failed > 0 { if zh { format!("{} 个账号刷新失败，已保留缓存", stats.failed) } else { format!("{} accounts could not refresh", stats.failed) } }
                                else if zh { "全部额度已刷新".into() } else { "All quotas refreshed".into() }
                            }),
                            Action::Cancel(id) => modules::auto_switch::cancel_auto_switch(app.clone(), id).map(|_| if zh { "待切换操作已取消".into() } else { "Pending switch cancelled".into() }),
                            _ => Ok(String::new()),
                        };
                        if let Ok(mut notice) = NOTICE.lock() { *notice = Some(result.unwrap_or_else(|_| if zh { "操作失败，请在 App 中检查账号状态".into() } else { "Operation failed. Check this account in the app.".into() })); }
                        BUSY.store(false, Ordering::Release);
                        modules::tray::update_tray_menus(&app);
                    });
                }
            }
        }
    }
);
impl MenuAction {
    fn new(marker: MainThreadMarker, state: ActionState) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(state);
        // NSObject initializer; targets stay retained throughout menu tracking.
        unsafe { msg_send![super(this), init] }
    }
}

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    struct SectionView;
    unsafe impl NSObjectProtocol for SectionView {}
    impl SectionView { #[unsafe(method(isFlipped))] fn flipped(&self) -> bool { true } }
);
fn section(marker: MainThreadMarker, height: f64) -> Retained<SectionView> {
    unsafe { msg_send![SectionView::alloc(marker), initWithFrame: rect(0.0, 0.0, WIDTH, height)] }
}
struct BarState { value: Option<f64>, preferences: MenuBarPreferences }
define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = BarState]
    struct QuotaBar;
    unsafe impl NSObjectProtocol for QuotaBar {}
    impl QuotaBar {
        #[unsafe(method(drawRect:))]
        fn draw(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            NSColor::quaternaryLabelColor().setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bounds, 3.0, 3.0).fill();
            if let Some(value) = self.ivars().value {
                let color = quota_color(Some(value), &self.ivars().preferences);
                color.setFill();
                let fill = rect(0.0, 0.0, bounds.size.width * (value / 100.0).clamp(0.0, 1.0), bounds.size.height);
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(fill, 3.0, 3.0).fill();
            }
        }
    }
);
fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect { NSRect::new(NSPoint::new(x, y), NSSize::new(width, height)) }
fn label(view: &NSView, text: &str, x: f64, y: f64, width: f64, size: f64, bold: bool, secondary: bool, marker: MainThreadMarker) -> Retained<NSTextField> {
    let field = NSTextField::labelWithString(&NSString::from_str(text), marker);
    field.setFrame(rect(x, y, width, 18.0));
    let font = if bold { NSFont::boldSystemFontOfSize(size) } else { NSFont::systemFontOfSize(size) };
    let color = if secondary { NSColor::secondaryLabelColor() } else { NSColor::labelColor() };
    field.setFont(Some(&font)); field.setTextColor(Some(&color));
    field.setSelectable(false);
    view.addSubview(&field);
    field
}
fn quota_color(value: Option<f64>, preferences: &MenuBarPreferences) -> Retained<NSColor> {
    match projection::quota_tone(value, preferences) { projection::QuotaTone::Healthy => NSColor::systemGreenColor(),
        projection::QuotaTone::Warning => NSColor::systemYellowColor(), projection::QuotaTone::Critical => NSColor::systemRedColor(),
        projection::QuotaTone::Unknown => NSColor::secondaryLabelColor() }
}
fn bar(view: &NSView, value: Option<f64>, preferences: &MenuBarPreferences, frame: NSRect, marker: MainThreadMarker) {
    let this = QuotaBar::alloc(marker).set_ivars(BarState { value, preferences: preferences.clone() });
    let progress: Retained<QuotaBar> = unsafe { msg_send![super(this), initWithFrame: frame] };
    view.addSubview(&progress);
}
fn symbol(name: &str) -> Option<Retained<NSImage>> { NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(name), None) }
fn image(view: &NSView, image: Option<Retained<NSImage>>, frame: NSRect, marker: MainThreadMarker) {
    if let Some(image) = image { let field = NSImageView::new(marker); field.setFrame(frame); field.setImage(Some(&image)); view.addSubview(&field); }
}
fn button(view: &NSView, menu: &NSMenu, app: &tauri::AppHandle, title: &str, action: Action, enabled: bool, frame: NSRect, icon: Option<&str>, zh: bool, targets: &mut Vec<Retained<MenuAction>>, marker: MainThreadMarker) -> Retained<NSButton> {
    let target = MenuAction::new(marker, ActionState { app: app.clone(), menu: menu.into(), action, zh });
    let button = unsafe { NSButton::buttonWithTitle_target_action(&NSString::from_str(title), Some(&target), Some(sel!(perform:)), marker) };
    button.setFrame(frame); button.setFont(Some(&NSFont::systemFontOfSize(11.0))); button.setBordered(true);
    button.setBezelStyle(NSBezelStyle::Push); button.setControlSize(NSControlSize::Small); button.setEnabled(enabled);
    if let Some(image) = icon.and_then(symbol) { button.setImage(Some(&image)); button.setImagePosition(NSCellImagePosition::ImageLeading); }
    view.addSubview(&button); targets.push(target); button
}
fn custom_item(menu: &NSMenu, view: &NSView, title: &str, marker: MainThreadMarker) -> Retained<NSMenuItem> {
    let item = unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(marker), &NSString::from_str(title), None, &NSString::from_str("")) };
    item.setView(Some(view)); menu.addItem(&item); item
}
fn standard_item(menu: &NSMenu, app: &tauri::AppHandle, title: &str, action: Action, enabled: bool, key: &str, zh: bool, targets: &mut Vec<Retained<MenuAction>>, marker: MainThreadMarker) {
    let target = MenuAction::new(marker, ActionState { app: app.clone(), menu: menu.into(), action, zh });
    let item = unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(marker), &NSString::from_str(title), Some(sel!(perform:)), &NSString::from_str(key)) };
    unsafe { item.setTarget(Some(&target)); }
    item.setEnabled(enabled); menu.addItem(&item); targets.push(target);
}
fn readonly_item(menu: &NSMenu, title: &str, marker: MainThreadMarker) {
    let item = unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(marker), &NSString::from_str(title), None, &NSString::from_str("")) };
    item.setEnabled(false); menu.addItem(&item);
}
fn can_switch(account: &DashboardEntry, now: i64) -> bool {
    account.read_status == "loaded" && !account.disabled && !account.quota.as_ref().is_some_and(|quota| quota.is_forbidden)
        && !(account.validation_blocked && account.validation_blocked_until.is_none_or(|until| until > now))
}

fn account_item(menu: &NSMenu, app: &tauri::AppHandle, account: &DashboardEntry, windows: [[Option<f64>; 2]; 2], preferences: &MenuBarPreferences, current: bool, busy: bool, verified: bool, zh: bool, targets: &mut Vec<Retained<MenuAction>>, marker: MainThreadMarker) {
    let (primary, secondary) = projection::identity_parts(account, preferences);
    let title = if secondary.is_empty() { primary.clone() } else { format!("{primary} · {secondary}") };
    let periods: Vec<_> = (0..2).filter(|period| if *period == 0 { preferences.show_session } else { preferences.show_weekly }).collect();
    let view = section(marker, 30.0 + periods.len() as f64 * 18.0);
    let action_x = if preferences.actions_leading { 16.0 } else { WIDTH - 80.0 };
    let name_x = if preferences.actions_leading { 84.0 } else { 20.0 };
    let available_width = WIDTH - 100.0;
    let name = label(&view, &primary, name_x, 4.0, available_width, 12.0, true, account.disabled, marker);
    name.sizeToFit();
    let name_width = name.frame().size.width.min(available_width - if secondary.is_empty() { 0.0 } else { 70.0 });
    name.setFrame(rect(name_x, 4.0, name_width, 18.0));
    if !secondary.is_empty() { label(&view, &format!(" · {secondary}"), name_x + name_width, 5.0, available_width - name_width, 11.0, false, true, marker); }
    let action_title = if current { if verified { if zh { "当前" } else { "Current" } } else { if zh { "记录" } else { "Saved" } } } else { if zh { "切换" } else { "Switch" } };
    button(&view, menu, app, action_title, if current { Action::Noop } else { Action::Switch(account.id.clone()) }, !current && !busy && can_switch(account, chrono::Utc::now().timestamp()), rect(action_x, 0.0, 64.0, 26.0), Some(if current { "checkmark" } else { "arrow.left.arrow.right" }), zh, targets, marker);
    let families: &[usize] = match preferences.display_scope { MenuBarQuotaScope::All => &[0, 1], MenuBarQuotaScope::Gemini => &[0], MenuBarQuotaScope::Other => &[1] };
    for (row, &period) in periods.iter().enumerate() {
        let y = 29.0 + row as f64 * 18.0;
        label(&view, if period == 0 { if zh { "5 小时" } else { "5 hours" } } else { if zh { "每周" } else { "Weekly" } }, 20.0, y - 3.0, 48.0, 11.0, false, true, marker);
        for (column, &family) in families.iter().enumerate() {
            let column_width = (WIDTH - 90.0) / families.len() as f64;
            let x = 70.0 + column as f64 * column_width;
            let bar_width = column_width - 49.0;
            bar(&view, windows[period][family], preferences, rect(x, y + 3.0, bar_width, 4.0), marker);
            let field = label(&view, &projection::percent(windows[period][family]), x + bar_width + 7.0, y - 3.0, 42.0, 11.0, false, true, marker);
            field.setTextColor(Some(&quota_color(windows[period][family], preferences)));
        }
    }
    view.setToolTip(Some(&NSString::from_str(&format!("{} · Gemini / Claude-GPT", account.email))));
    let item = custom_item(menu, &view, &title, marker);
    let details = NSMenu::new(marker); details.setAutoenablesItems(false); details.setMinimumWidth(WIDTH);
    let heading = section(marker, 48.0);
    label(&heading, account.quota.as_ref().and_then(|quota| quota.subscription_tier.as_deref()).unwrap_or("Account"), 20.0, 4.0, WIDTH - 40.0, 13.0, true, false, marker);
    label(&heading, if zh { "缓存详情 · 最新额度以刷新结果为准" } else { "Cached details · refresh for current quotas" }, 20.0, 26.0, WIDTH - 40.0, 11.0, false, true, marker);
    custom_item(&details, &heading, "Account details", marker);
    details.addItem(&NSMenuItem::separatorItem(marker));
    if let Some(quota) = &account.quota {
        for group in quota.groups.iter().flatten() {
            let group_heading = section(marker, 30.0);
            let name = group.display_name.to_lowercase();
            let title = if name.contains("gemini") { "Gemini" } else if name.contains("claude") || name.contains("gpt") { "Claude / GPT" } else { &group.display_name };
            if preferences.show_icons { image(&group_heading, symbol(if name.contains("gemini") { "sparkles" } else { "brain" }), rect(20.0, 6.0, 16.0, 16.0), marker); }
            let x = if preferences.show_icons { 43.0 } else { 20.0 };
            label(&group_heading, title, x, 5.0, WIDTH - x - 20.0, 13.0, true, false, marker);
            custom_item(&details, &group_heading, title, marker);
            for bucket in &group.buckets {
                let row = section(marker, 48.0);
                let period = match bucket.window.as_str() { "5h" => if zh { "5 小时" } else { "5 hours" }, "weekly" => if zh { "每周" } else { "Weekly" }, _ => &bucket.window };
                let value = bucket.remaining_fraction.filter(|value| value.is_finite() && (0.0..=1.0).contains(value)).map(|value| value * 100.0);
                label(&row, period, 20.0, 2.0, 85.0, 12.0, false, false, marker);
                let field = label(&row, &projection::percent(value), WIDTH - 68.0, 2.0, 48.0, 12.0, true, false, marker);
                field.setTextColor(Some(&quota_color(value, preferences))); field.setAlignment(objc2_app_kit::NSTextAlignment::Right);
                label(&row, &projection::reset_summary(&bucket.reset_time, chrono::Utc::now().timestamp(), zh), 108.0, 3.0, WIDTH - 180.0, 11.0, false, true, marker);
                bar(&row, value, preferences, rect(20.0, 29.0, WIDTH - 40.0, 4.0), marker);
                row.setToolTip(Some(&NSString::from_str(&bucket.reset_time)));
                custom_item(&details, &row, period, marker);
            }
        }
        if quota.groups.as_ref().is_none_or(|groups| groups.is_empty()) {
            for model in &quota.models {
                let row = section(marker, 50.0);
                label(&row, model.display_name.as_ref().unwrap_or(&model.name), 20.0, 2.0, WIDTH - 95.0, 12.0, false, false, marker);
                let field = label(&row, &projection::percent(model.percentage), WIDTH - 68.0, 2.0, 48.0, 12.0, true, false, marker);
                field.setTextColor(Some(&quota_color(model.percentage, preferences)));
                label(&row, &projection::reset_summary(&model.reset_time, chrono::Utc::now().timestamp(), zh), 20.0, 25.0, WIDTH - 40.0, 11.0, false, true, marker);
                custom_item(&details, &row, &model.name, marker);
            }
        }
    } else {
        let row = section(marker, 30.0);
        label(&row, if zh { "额度未报告" } else { "Quota not reported" }, 20.0, 5.0, WIDTH - 40.0, 12.0, false, false, marker);
        custom_item(&details, &row, "Quota not reported", marker);
    }
    details.addItem(&NSMenuItem::separatorItem(marker));
    standard_item(&details, app, if zh { "在 App 中管理账号…" } else { "Manage accounts in app…" }, Action::Page("accounts"), true, "", zh, targets, marker);
    item.setSubmenu(Some(&details));
}

fn show(app: tauri::AppHandle, config: AppConfig, snapshot: Option<DashboardSnapshot>, status: Option<modules::auto_switch::Status>, reserve: u8, _anchor: Option<tauri::Rect>, ticket: u64) {
    let Some(marker) = MainThreadMarker::new() else { OPEN.store(false, Ordering::Release); return; };
    if !OPEN.load(Ordering::Acquire) || GENERATION.load(Ordering::Acquire) != ticket { return; }
    let zh = config.language.starts_with("zh");
    let menu = NSMenu::new(marker); menu.setAutoenablesItems(false); menu.setMinimumWidth(WIDTH);
    let mut targets = Vec::new();
    let now = chrono::Utc::now().timestamp();
    let preferences = &config.menu_bar;
    let accounts: Vec<_> = snapshot.as_ref().map(|snapshot| snapshot.accounts.iter().filter(|account| projection::visible_account(account, preferences, now)).collect()).unwrap_or_default();
    let windows: Vec<_> = accounts.iter().map(|account| projection::account_windows(account, now, config.refresh_interval)).collect();
    let scope = config.menu_bar.quota_scope;
    let scope_name = match scope { crate::models::config::MenuBarQuotaScope::All => if zh { "全部系列" } else { "All families" }, crate::models::config::MenuBarQuotaScope::Gemini => "Gemini", crate::models::config::MenuBarQuotaScope::Other => "Claude / GPT" };
    let header = section(marker, 40.0);
    if preferences.show_icons { image(&header, NSApplication::sharedApplication(marker).applicationIconImage(), rect(20.0, 5.0, 24.0, 24.0), marker); }
    let title_x = if preferences.show_icons { 53.0 } else { 20.0 };
    label(&header, BRAND, title_x, 3.0, WIDTH - title_x - 20.0, 13.0, true, false, marker);
    label(&header, if zh { "额度总览" } else { "Quota overview" }, title_x, 22.0, 240.0, 11.0, false, true, marker);
    custom_item(&menu, &header, BRAND, marker);
    menu.addItem(&NSMenuItem::separatorItem(marker));
    if preferences.show_aggregate {
      let heading = section(marker, 29.0);
      label(&heading, if zh { "整体额度" } else { "Overall quotas" }, 20.0, 5.0, 110.0, 13.0, true, false, marker);
      label(&heading, &format!("{scope_name} · {}", if zh { "平均剩余" } else { "Mean remaining" }), 144.0, 7.0, WIDTH - 164.0, 11.0, false, true, marker);
      custom_item(&menu, &heading, "Overall quotas", marker);
    for period in (0..2).filter(|period| if *period == 0 { preferences.show_session } else { preferences.show_weekly }) {
        let (remaining, usable, covered) = projection::aggregate(&windows, scope, period, reserve);
        let view = section(marker, 48.0);
        label(&view, if period == 0 { if zh { "5 小时" } else { "5 hours" } } else { if zh { "每周" } else { "Weekly" } }, 20.0, 5.0, 85.0, 13.0, true, false, marker);
        let stats = label(&view, &format!("{} {usable}/{} · {} {}", if zh { "可用账号" } else { "Available" }, accounts.len(), if zh { "剩余" } else { "Left" }, projection::percent(remaining)), 118.0, 7.0, WIDTH - 138.0, 11.0, false, true, marker);
        stats.setAlignment(objc2_app_kit::NSTextAlignment::Right);
        bar(&view, remaining, preferences, rect(20.0, 30.0, WIDTH - 40.0, 6.0), marker);
        view.setToolTip(Some(&NSString::from_str(&format!("{covered}/{} {}", accounts.len(), if zh { "账号有完整有效数据；按账号等权平均，不代表 Token 总量" } else { "accounts report valid data; an equal-weight mean, not token capacity" }))));
        custom_item(&menu, &view, if period == 0 { "5 hours" } else { "Weekly" }, marker);
    }
    menu.addItem(&NSMenuItem::separatorItem(marker));
    }
    let account_header = section(marker, 62.0);
    label(&account_header, &format!("{} · {}", if zh { "账号" } else { "Accounts" }, accounts.len()), 20.0, 4.0, 160.0, 13.0, true, false, marker);
    for (index, scope) in [MenuBarQuotaScope::Gemini, MenuBarQuotaScope::Other, MenuBarQuotaScope::All].into_iter().enumerate() {
        let title = match scope { MenuBarQuotaScope::Gemini => "Gemini", MenuBarQuotaScope::Other => if zh { "非 Gemini" } else { "Non-Gemini" }, MenuBarQuotaScope::All => if zh { "全部" } else { "Both" } };
        let icon = if !preferences.show_icons { None } else { Some(match scope { MenuBarQuotaScope::Gemini => "sparkles", MenuBarQuotaScope::Other => "brain", MenuBarQuotaScope::All => "square.grid.2x2" }) };
        let filter = button(&account_header, &menu, &app, title, Action::Filter(scope), true, rect(16.0 + index as f64 * 116.0, 28.0, 112.0, 27.0), icon, zh, &mut targets, marker);
        filter.setButtonType(NSButtonType::PushOnPushOff);
        if preferences.display_scope == scope { filter.setState(NSControlStateValueOn); }
    }
    custom_item(&menu, &account_header, "Accounts", marker);
    let busy = BUSY.load(Ordering::Acquire) || status.as_ref().is_none_or(|status| status.phase == "switching");
    if snapshot.is_none() { readonly_item(&menu, if zh { "账号读取失败，请重试" } else { "Could not read accounts. Retry." }, marker); }
    else if accounts.is_empty() { readonly_item(&menu, if zh { "尚未添加账号" } else { "No saved accounts" }, marker); }
    for (account, windows) in accounts.iter().zip(windows) {
        let current = snapshot.as_ref().and_then(|snapshot| snapshot.current_account_id.as_ref()).is_some_and(|id| id == &account.id);
        let verified = snapshot.as_ref().is_some_and(|snapshot| snapshot.current_identity_source == "running_app");
        account_item(&menu, &app, account, windows, preferences, current, busy, verified, zh, &mut targets, marker);
    }
    menu.addItem(&NSMenuItem::separatorItem(marker));
    if let Ok(notice) = NOTICE.lock() { if let Some(notice) = notice.as_ref() { readonly_item(&menu, notice, marker); } }
    if BUSY.load(Ordering::Acquire) { readonly_item(&menu, if zh { "正在执行，请稍候…" } else { "Working…" }, marker); }
    if let Some(status) = status.filter(|status| status.pending_id.is_some()) {
        readonly_item(&menu, if zh { "智能换号：等待客户端关闭" } else { "Auto switch: waiting for clients" }, marker);
        standard_item(&menu, &app, if zh { "取消待切换操作" } else { "Cancel pending switch" }, Action::Cancel(status.pending_id.unwrap()), !busy, "", zh, &mut targets, marker);
    }
    standard_item(&menu, &app, if zh { "刷新全部额度" } else { "Refresh All Quotas" }, Action::Refresh, !busy, "r", zh, &mut targets, marker);
    standard_item(&menu, &app, if zh { "用量看板…" } else { "Usage Dashboard…" }, Action::Page("dashboard"), true, "", zh, &mut targets, marker);
    standard_item(&menu, &app, if zh { "管理账号…" } else { "Manage Accounts…" }, Action::Page("accounts"), true, "", zh, &mut targets, marker);
    menu.addItem(&NSMenuItem::separatorItem(marker));
    standard_item(&menu, &app, if zh { "设置…" } else { "Settings…" }, Action::Page("settings"), true, ",", zh, &mut targets, marker);
    standard_item(&menu, &app, if zh { "关于 AntiGravity tool lite…" } else { "About AntiGravity tool lite…" }, Action::About, true, "", zh, &mut targets, marker);
    standard_item(&menu, &app, "GitHub ↗", Action::Github, true, "", zh, &mut targets, marker);
    standard_item(&menu, &app, if zh { "退出" } else { "Quit" }, Action::Quit, true, "q", zh, &mut targets, marker);
    ACTIVE.with(|active| *active.borrow_mut() = Some(menu.clone()));
    // Associate the NSMenu with the real status item. AppKit then owns the
    // anchor, active screen, accessibility hierarchy and native menu tracking.
    // This synchronous closure runs on the main thread while `menu` is retained
    // here; only its address crosses Tauri's Send bound, never another thread.
    let menu_address = Retained::as_ptr(&menu) as usize;
    let shown = app.tray_by_id("main").and_then(|tray| tray.with_inner_tray_icon(move |icon| {
        let marker = MainThreadMarker::new().expect("tray callback is on main thread");
        let menu = unsafe { &*(menu_address as *const NSMenu) };
        let Some(status_item) = icon.ns_status_item() else { return false; };
        let previous = status_item.menu(marker);
        status_item.setMenu(Some(menu));
        // tray-icon overlays its own mouse view on the status button. Open
        // from the status item itself instead of synthesizing a button click.
        #[allow(deprecated)]
        status_item.popUpStatusItemMenu(menu);
        status_item.setMenu(previous.as_deref());
        true
    }).ok()).unwrap_or(false);
    if !shown { menu.popUpMenuPositioningItem_atLocation_inView(None, objc2_app_kit::NSEvent::mouseLocation(), None); }
    ACTIVE.with(|active| active.borrow_mut().take());
    if GENERATION.load(Ordering::Acquire) == ticket { OPEN.store(false, Ordering::Release); }
    drop(targets);
    if REOPEN.swap(false, Ordering::AcqRel) { let _ = toggle(&app, None); }
}

pub fn toggle(app: &tauri::AppHandle, anchor: Option<tauri::Rect>) -> Result<(), String> {
    let ticket = GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    if OPEN.swap(true, Ordering::AcqRel) {
        OPEN.store(false, Ordering::Release);
        return app.run_on_main_thread(|| ACTIVE.with(|active| { if let Some(menu) = active.borrow().as_ref() { menu.cancelTrackingWithoutAnimation(); } })).map_err(|error| error.to_string());
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let snapshot = commands::get_menu_bar_snapshot().await.ok();
        if !OPEN.load(Ordering::Acquire) || GENERATION.load(Ordering::Acquire) != ticket { return; }
        let config = modules::load_app_config().unwrap_or_default();
        let status = modules::auto_switch::get_auto_switch_status(app.clone()).ok();
        let reserve = modules::auto_switch::get_auto_switch_config(app.clone()).map(|config| config.reserve_percentage).unwrap_or(10);
        let handle = app.clone();
        if handle.run_on_main_thread(move || show(app, config, snapshot, status, reserve, anchor, ticket)).is_err() { OPEN.store(false, Ordering::Release); }
    });
    Ok(())
}
