//! macOS overview is an NSMenu, like CodexBar: AppKit owns the surface and
//! tracking; custom content uses semantic system text and small quota bars.
//! No WebView transparency, root-view replacement, or credential DTOs.
use super::{account_dashboard::{DashboardSnapshot, DashboardEntry}, menu_bar_projection as projection};
use crate::{commands, modules, models::AppConfig};
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadOnly};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSApplication, NSBezierPath, NSButton, NSBezelStyle, NSCellImagePosition, NSControlSize, NSColor, NSFont, NSImage, NSImageView, NSMenu, NSMenuItem, NSTextField, NSView};
use crate::models::config::{MenuBarPreferences, MenuBarQuotaScope};
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
static NOTICE: Mutex<Option<String>> = Mutex::new(None);
struct MenuPages {
    overview: Vec<Retained<NSMenuItem>>,
    details: Vec<(String, Vec<Retained<NSMenuItem>>)>,
}
thread_local! {
    static ACTIVE: RefCell<Option<Retained<NSMenu>>> = const { RefCell::new(None) };
    static PAGES: RefCell<Option<MenuPages>> = const { RefCell::new(None) };
}
fn show_page(menu: &NSMenu, account_id: Option<&str>) {
    PAGES.with(|pages| {
        let pages = pages.borrow();
        let Some(pages) = pages.as_ref() else { return; };
        if account_id.is_some_and(|id| !pages.details.iter().any(|(key, _)| key == id)) { return; }
        for item in &pages.overview { item.setHidden(account_id.is_some()); }
        for (id, items) in &pages.details {
            for item in items { item.setHidden(account_id != Some(id.as_str())); }
        }
    });
    // Update this tracking session; do not dismiss/reopen or create a submenu.
    menu.update();
}

#[derive(Clone)]
enum Action { Switch(String), Details(String), Overview, Page(&'static str), Refresh, Cancel(String), Github, Quit }
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
            match &state.action {
                Action::Details(id) => { show_page(&state.menu, Some(id)); return; },
                Action::Overview => { show_page(&state.menu, None); return; },
                _ => {},
            }
            let mut root = state.menu.clone();
            // All parent menus are retained during this tracking session.
            while let Some(parent) = unsafe { root.supermenu() } { root = parent; }
            root.cancelTrackingWithoutAnimation();
            let app = state.app.clone();
            let zh = state.zh;
            match &state.action {
                Action::Quit => app.exit(0),
                Action::Details(_) | Action::Overview => unreachable!(),
                Action::Github => { let _ = app.opener().open_url(GITHUB, None::<&str>); },
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
struct BarState { value: Option<f64>, preferences: MenuBarPreferences, disabled: bool }
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
            if self.ivars().disabled { NSColor::systemRedColor().setFill(); } else { NSColor::quaternaryLabelColor().setFill(); }
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
// Lay out quota columns according to the family selected in Settings.
struct QuotaRow { cells: Vec<(Retained<QuotaBar>, Retained<NSTextField>)>, y: f64 }
impl QuotaRow {
    fn apply(&self, scope: MenuBarQuotaScope) {
        let width = (WIDTH - 90.0) / if scope == MenuBarQuotaScope::All { 2.0 } else { 1.0 };
        for (family, (progress, text)) in self.cells.iter().enumerate() {
            let visible = scope == MenuBarQuotaScope::All || (scope == MenuBarQuotaScope::Gemini && family == 0) || (scope == MenuBarQuotaScope::Other && family == 1);
            progress.setHidden(!visible); text.setHidden(!visible);
            let x = 70.0 + if scope == MenuBarQuotaScope::All { family as f64 * width } else { 0.0 };
            progress.setFrame(rect(x, self.y + 3.0, width - 49.0, 4.0)); progress.setNeedsDisplay(true);
            text.setFrame(rect(x + width - 44.0, self.y - 3.0, 46.0, 18.0));
        }
    }
}
define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    struct AccountRow;
    unsafe impl NSObjectProtocol for AccountRow {}
    impl AccountRow {
        #[unsafe(method(isFlipped))] fn flipped(&self) -> bool { true }
        #[unsafe(method(drawRect:))]
        fn draw(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            NSColor::separatorColor().colorWithAlphaComponent(0.45).setFill();
            NSBezierPath::bezierPathWithRect(rect(20.0, bounds.size.height - 2.0, bounds.size.width - 40.0, 0.5)).fill();
        }
    }
);
fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect { NSRect::new(NSPoint::new(x, y), NSSize::new(width, height)) }
fn label(view: &NSView, text: &str, x: f64, y: f64, width: f64, size: f64, bold: bool, secondary: bool, marker: MainThreadMarker) -> Retained<NSTextField> {
    let field = NSTextField::labelWithString(&NSString::from_str(text), marker);
    // NSTextField adds two points of horizontal cell padding. Compensate so
    // text, bars and action controls share the same content edges.
    field.setFrame(rect(x - 2.0, y, width + 4.0, 18.0));
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
fn bar(view: &NSView, value: Option<f64>, preferences: &MenuBarPreferences, frame: NSRect, disabled: bool, marker: MainThreadMarker) -> Retained<QuotaBar> {
    let this = QuotaBar::alloc(marker).set_ivars(BarState { value, preferences: preferences.clone(), disabled });
    let progress: Retained<QuotaBar> = unsafe { msg_send![super(this), initWithFrame: frame] };
    view.addSubview(&progress); progress
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
// Read-only status chips keep their semantic color instead of AppKit's
// disabled-control dimming. They share the switch button's layout footprint.
define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    struct StatusBadge;
    unsafe impl NSObjectProtocol for StatusBadge {}
    impl StatusBadge {
        #[unsafe(method(isFlipped))] fn flipped(&self) -> bool { true }
        #[unsafe(method(drawRect:))]
        fn draw(&self, _dirty: NSRect) {
            let shape = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect(0.5, 3.5, self.bounds().size.width - 1.0, 19.0), 5.0, 5.0);
            NSColor::quaternaryLabelColor().setFill(); shape.fill();
        }
    }
);
fn status_badge(view: &NSView, title: &str, icon: &str, color: &NSColor, frame: NSRect, marker: MainThreadMarker) {
    let badge: Retained<StatusBadge> = unsafe { msg_send![StatusBadge::alloc(marker), initWithFrame: frame] };
    let text_width = if title.is_ascii() { title.len() as f64 * 5.5 } else { title.chars().count() as f64 * 11.0 };
    let glyph_x = (frame.size.width - 19.0 - text_width) / 2.0;
    let glyph = NSImageView::new(marker); glyph.setFrame(rect(glyph_x, 6.0, 14.0, 14.0));
    glyph.setImage(symbol(icon).as_deref()); glyph.setContentTintColor(Some(color)); badge.addSubview(&glyph);
    let text = label(&badge, title, glyph_x + 19.0, 5.0, text_width, 11.0, false, false, marker);
    text.setTextColor(Some(color)); view.addSubview(&badge);
}
struct UsageRingState { values: [f64; 3] }
define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = UsageRingState]
    struct UsageRing;
    unsafe impl NSObjectProtocol for UsageRing {}
    impl UsageRing {
        #[unsafe(method(isFlipped))] fn flipped(&self) -> bool { true }
        #[unsafe(method(drawRect:))]
        fn draw(&self, _dirty: NSRect) {
            let size = self.bounds().size.width;
            let track = NSBezierPath::bezierPathWithOvalInRect(rect(4.0, 4.0, size - 8.0, size - 8.0));
            NSColor::quaternaryLabelColor().setStroke(); track.setLineWidth(8.0); track.stroke();
            let total: f64 = self.ivars().values.iter().sum();
            if total <= 0.0 { return; }
            let mut angle = -90.0;
            for (index, value) in self.ivars().values.iter().enumerate() {
                if *value <= 0.0 { continue; }
                let next = angle + value / total * 360.0;
                let arc = NSBezierPath::bezierPath();
                usage_color(index).setStroke(); arc.setLineWidth(8.0);
                arc.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(NSPoint::new(size / 2.0, size / 2.0), (size - 8.0) / 2.0, angle, next, false);
                arc.stroke(); angle = next;
            }
        }
    }
);
fn usage_color(index: usize) -> Retained<NSColor> {
    match index { 0 => NSColor::systemBlueColor(), 1 => NSColor::systemOrangeColor(), _ => NSColor::systemPurpleColor() }
}
fn compact_tokens(value: u64) -> String {
    if value >= 1_000_000 { format!("{:.1}M", value as f64 / 1_000_000.0) }
    else if value >= 1_000 { format!("{:.1}K", value as f64 / 1_000.0) }
    else { value.to_string() }
}
fn usage_section(menu: &NSMenu, usage: Option<&modules::menu_bar_usage::MenuBarUsage>, zh: bool, marker: MainThreadMarker) {
    let heading = section(marker, 29.0);
    label(&heading, if zh { "今日用量" } else { "Today's usage" }, 20.0, 5.0, 160.0, 13.0, true, false, marker);
    let scope = label(&heading, if usage.is_some_and(|usage| usage.incomplete) { if zh { "统计不完整" } else { "Partial records" } } else { if zh { "本机" } else { "Local" } }, 210.0, 7.0, WIDTH - 230.0, 11.0, false, true, marker);
    scope.setAlignment(objc2_app_kit::NSTextAlignment::Right);
    custom_item(menu, &heading, "Today's usage", marker);
    let view = section(marker, 119.0);
    let values = usage.map(|usage| [usage.today.input_tokens as f64, usage.today.output_tokens as f64, usage.today.cached_tokens as f64]).unwrap_or([0.0; 3]);
    let ring = UsageRing::alloc(marker).set_ivars(UsageRingState { values });
    let ring: Retained<UsageRing> = unsafe { msg_send![super(ring), initWithFrame: rect(20.0, 5.0, 88.0, 88.0)] };
    view.addSubview(&ring);
    let total = label(&view, &usage.map(|usage| compact_tokens(usage.today.total_tokens)).unwrap_or("—".into()), 25.0, 33.0, 78.0, 17.0, true, false, marker);
    total.setAlignment(objc2_app_kit::NSTextAlignment::Center);
    let unit = label(&view, "tokens", 25.0, 53.0, 78.0, 9.0, false, true, marker);
    unit.setAlignment(objc2_app_kit::NSTextAlignment::Center);
    let requests = label(&view, &usage.map(|usage| format!("{} {}", usage.today.request_count, if zh { "次请求" } else { "requests" })).unwrap_or("—".into()), 20.0, 100.0, 88.0, 10.0, false, true, marker);
    requests.setAlignment(objc2_app_kit::NSTextAlignment::Center);
    label(&view, if zh { "API 费用估算" } else { "API estimate" }, 130.0, 5.0, 140.0, 11.0, false, true, marker);
    let amount = usage.and_then(|usage| usage.estimated_usd).map(|usd| if usd > 0.0 && usd < 0.01 { format!("$ {usd:.4}") } else { format!("$ {usd:.2}") }).unwrap_or(if usage.is_some() { if zh { "未计价".into() } else { "Unpriced".into() } } else { "—".into() });
    let cost = label(&view, &amount, WIDTH - 105.0, 4.0, 85.0, 13.0, true, false, marker);
    cost.setAlignment(objc2_app_kit::NSTextAlignment::Right);
    for (index, name) in [if zh { "输入" } else { "Input" }, if zh { "输出" } else { "Output" }, if zh { "缓存" } else { "Cached" }].iter().enumerate() {
        let y = 32.0 + index as f64 * 21.0;
        label(&view, name, 142.0, y, 90.0, 11.0, false, true, marker).setTextColor(Some(&usage_color(index)));
        let value = label(&view, &usage.map(|_| compact_tokens(values[index] as u64)).unwrap_or("—".into()), WIDTH - 105.0, y, 85.0, 11.0, false, false, marker);
        value.setAlignment(objc2_app_kit::NSTextAlignment::Right);
    }
    let status = usage.map(|usage| if usage.unpriced_models > 0 { if zh { "部分未计价" } else { "Partly unpriced" } } else if usage.pricing_stale && usage.today.total_tokens > 0 { if zh { "缓存价格" } else { "Cached prices" } } else { "USD" }).unwrap_or(if zh { "统计暂不可用" } else { "Usage unavailable" });
    let status = label(&view, status, 130.0, 100.0, WIDTH - 150.0, 10.0, false, true, marker);
    status.setAlignment(objc2_app_kit::NSTextAlignment::Right);
    custom_item(menu, &view, "Local usage breakdown", marker);
    menu.addItem(&NSMenuItem::separatorItem(marker));
}
fn custom_item(menu: &NSMenu, view: &NSView, title: &str, marker: MainThreadMarker) -> Retained<NSMenuItem> {
    let item = unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(marker), &NSString::from_str(title), None, &NSString::from_str("")) };
    item.setView(Some(view)); menu.addItem(&item); item
}
fn standard_item(menu: &NSMenu, app: &tauri::AppHandle, title: &str, action: Action, enabled: bool, key: &str, show_icons: bool, zh: bool, targets: &mut Vec<Retained<MenuAction>>, marker: MainThreadMarker) {
    let icon = if show_icons { match &action {
        Action::Page("dashboard") => Some("chart.bar"), Action::Page("accounts") => Some("person.2"), Action::Page("settings") => Some("gearshape"),
        Action::Refresh => Some("arrow.clockwise"), Action::Github => Some("link"), Action::Quit => Some("power"), Action::Cancel(_) => Some("xmark.circle"), _ => None,
    } } else { None };
    let item = unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(marker), &NSString::from_str(title), Some(sel!(perform:)), &NSString::from_str(key)) };
    if let Some(image) = icon.and_then(symbol) { image.setSize(NSSize::new(16.0, 16.0)); item.setImage(Some(&image)); }
    let target = MenuAction::new(marker, ActionState { app: app.clone(), menu: menu.into(), action, zh });
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
    let title = if secondary.is_empty() { primary.clone() } else { format!("{primary}   {secondary}") };
    let periods: Vec<_> = (0..2).filter(|period| if *period == 0 { preferences.show_session } else { preferences.show_weekly }).collect();
    let view: Retained<AccountRow> = unsafe { msg_send![AccountRow::alloc(marker), initWithFrame: rect(0.0, 0.0, WIDTH, 48.0 + periods.len() as f64 * 18.0)] };
    let action_width = if zh { 64.0 } else { 72.0 };
    let action_frame = rect(WIDTH - 20.0 - action_width, 8.0, action_width, 26.0);
    label(&view, &primary, 20.0, 7.0, WIDTH - action_width - 48.0, 12.0, true, false, marker);
    if !secondary.is_empty() { label(&view, &secondary, 20.0, 25.0, WIDTH - action_width - 48.0, 10.0, false, true, marker); }
    // A transparent hit target preserves ordinary text alignment and color.
    let identity = button(&view, menu, app, "", Action::Details(account.id.clone()), true, rect(20.0, 4.0, WIDTH - action_width - 48.0, 38.0), None, zh, targets, marker);
    identity.setBordered(false);
    unsafe { let _: () = msg_send![&*identity, setAccessibilityLabel: &*NSString::from_str(&format!("{} {primary}", if zh { "查看账号详情" } else { "Account details" }))]; }
    if account.disabled {
        status_badge(&view, if zh { "禁用" } else { "Disabled" }, "nosign", &NSColor::systemRedColor(), action_frame, marker);
    } else if current {
        status_badge(&view, if verified { if zh { "当前" } else { "Current" } } else { if zh { "记录" } else { "Saved" } }, "checkmark.circle", &NSColor::systemBlueColor(), action_frame, marker);
    } else {
        button(&view, menu, app, if zh { "切换" } else { "Switch" }, Action::Switch(account.id.clone()), !busy && can_switch(account, chrono::Utc::now().timestamp()), action_frame, Some("arrow.left.arrow.right"), zh, targets, marker);
    }
    // Disabled rows display usable quota as zero without changing the cached
    // observations or the aggregate calculation, which still excludes them.
    let windows = if account.disabled { [[Some(0.0); 2]; 2] } else { windows };
    for (row, &period) in periods.iter().enumerate() {
        let y = 46.0 + row as f64 * 18.0;
        label(&view, if period == 0 { if zh { "5 小时" } else { "5 hours" } } else { if zh { "每周" } else { "Weekly" } }, 20.0, y - 3.0, 48.0, 11.0, false, true, marker);
        let cells = (0..2).map(|family| {
            let progress = bar(&view, windows[period][family], preferences, rect(70.0, y + 3.0, 100.0, 4.0), account.disabled, marker);
            let field = label(&view, &projection::percent(windows[period][family]), 180.0, y - 3.0, 42.0, 11.0, false, true, marker);
            let color = NSColor::labelColor();
            field.setTextColor(Some(&color)); field.setAlignment(objc2_app_kit::NSTextAlignment::Right);
            (progress, field)
        }).collect();
        let row = QuotaRow { cells, y }; row.apply(preferences.display_scope);
    }
    custom_item(menu, &view, &title, marker);
}

fn account_details(menu: &NSMenu, app: &tauri::AppHandle, account: &DashboardEntry, preferences: &MenuBarPreferences, zh: bool, targets: &mut Vec<Retained<MenuAction>>, marker: MainThreadMarker) {
    let navigation = section(marker, 34.0);
    let back = button(&navigation, menu, app, if zh { "返回" } else { "Back" }, Action::Overview, true, rect(16.0, 4.0, 70.0, 26.0), Some("chevron.left"), zh, targets, marker);
    back.setBordered(false);
    label(&navigation, if zh { "账号详情" } else { "Account details" }, 100.0, 8.0, WIDTH - 120.0, 12.0, true, false, marker);
    custom_item(menu, &navigation, "Back to overview", marker);
    let identity = section(marker, 46.0);
    let (primary, secondary) = projection::identity_parts(account, preferences);
    label(&identity, &primary, 20.0, 3.0, WIDTH - 40.0, 13.0, true, false, marker);
    if !secondary.is_empty() { label(&identity, &secondary, 20.0, 24.0, WIDTH - 40.0, 11.0, false, true, marker); }
    custom_item(menu, &identity, "Account identity", marker);
    let heading = section(marker, 48.0);
    label(&heading, account.quota.as_ref().and_then(|quota| quota.subscription_tier.as_deref()).unwrap_or("Account"), 20.0, 4.0, WIDTH - 40.0, 13.0, true, false, marker);
    label(&heading, if zh { "缓存详情   最新额度以刷新结果为准" } else { "Cached details   refresh for current quotas" }, 20.0, 26.0, WIDTH - 40.0, 11.0, false, true, marker);
    custom_item(menu, &heading, "Account details", marker);
    menu.addItem(&NSMenuItem::separatorItem(marker));
    if account.disabled {
        for (title, icon) in [("Gemini", "sparkles"), ("Claude / GPT", "brain")] {
            let group_heading = section(marker, 30.0);
            if preferences.show_icons { image(&group_heading, symbol(icon), rect(20.0, 6.0, 16.0, 16.0), marker); }
            let x = if preferences.show_icons { 43.0 } else { 20.0 };
            label(&group_heading, title, x, 5.0, WIDTH - x - 20.0, 13.0, true, false, marker);
            custom_item(menu, &group_heading, title, marker);
            for period in [if zh { "每周" } else { "Weekly" }, if zh { "5 小时" } else { "5 hours" }] {
                let row = section(marker, 48.0);
                label(&row, period, 20.0, 2.0, 85.0, 12.0, false, false, marker);
                let field = label(&row, "0%", WIDTH - 68.0, 2.0, 48.0, 12.0, true, false, marker);
                field.setTextColor(Some(&NSColor::labelColor())); field.setAlignment(objc2_app_kit::NSTextAlignment::Right);
                label(&row, if zh { "禁用" } else { "Disabled" }, 108.0, 3.0, WIDTH - 180.0, 11.0, false, true, marker);
                bar(&row, Some(0.0), preferences, rect(20.0, 29.0, WIDTH - 40.0, 4.0), true, marker);
                custom_item(menu, &row, period, marker);
            }
        }
    } else if let Some(quota) = &account.quota {
        for group in quota.groups.iter().flatten() {
            let group_heading = section(marker, 30.0);
            let name = group.display_name.to_lowercase();
            let title = if name.contains("gemini") { "Gemini" } else if name.contains("claude") || name.contains("gpt") { "Claude / GPT" } else { &group.display_name };
            if preferences.show_icons { image(&group_heading, symbol(if name.contains("gemini") { "sparkles" } else { "brain" }), rect(20.0, 6.0, 16.0, 16.0), marker); }
            let x = if preferences.show_icons { 43.0 } else { 20.0 };
            label(&group_heading, title, x, 5.0, WIDTH - x - 20.0, 13.0, true, false, marker);
            custom_item(menu, &group_heading, title, marker);
            for bucket in &group.buckets {
                let row = section(marker, 48.0);
                let period = match bucket.window.as_str() { "5h" => if zh { "5 小时" } else { "5 hours" }, "weekly" => if zh { "每周" } else { "Weekly" }, _ => &bucket.window };
                let value = bucket.remaining_fraction.filter(|value| value.is_finite() && (0.0..=1.0).contains(value)).map(|value| value * 100.0);
                label(&row, period, 20.0, 2.0, 85.0, 12.0, false, false, marker);
                let field = label(&row, &projection::percent(value), WIDTH - 68.0, 2.0, 48.0, 12.0, true, false, marker);
                field.setTextColor(Some(&NSColor::labelColor())); field.setAlignment(objc2_app_kit::NSTextAlignment::Right);
                label(&row, &projection::reset_summary(&bucket.reset_time, chrono::Utc::now().timestamp(), zh), 108.0, 3.0, WIDTH - 180.0, 11.0, false, true, marker);
                bar(&row, value, preferences, rect(20.0, 29.0, WIDTH - 40.0, 4.0), false, marker);
                custom_item(menu, &row, period, marker);
            }
        }
        if quota.groups.as_ref().is_none_or(|groups| groups.is_empty()) {
            for model in &quota.models {
                let row = section(marker, 50.0);
                label(&row, model.display_name.as_ref().unwrap_or(&model.name), 20.0, 2.0, WIDTH - 95.0, 12.0, false, false, marker);
                let field = label(&row, &projection::percent(model.percentage), WIDTH - 68.0, 2.0, 48.0, 12.0, true, false, marker);
                field.setTextColor(Some(&NSColor::labelColor()));
                label(&row, &projection::reset_summary(&model.reset_time, chrono::Utc::now().timestamp(), zh), 20.0, 25.0, WIDTH - 40.0, 11.0, false, true, marker);
                custom_item(menu, &row, &model.name, marker);
            }
        }
    } else {
        let row = section(marker, 30.0);
        label(&row, if zh { "额度未报告" } else { "Quota not reported" }, 20.0, 5.0, WIDTH - 40.0, 12.0, false, false, marker);
        custom_item(menu, &row, "Quota not reported", marker);
    }
    menu.addItem(&NSMenuItem::separatorItem(marker));
    standard_item(menu, app, if zh { "在 App 中管理账号" } else { "Manage accounts in app" }, Action::Page("accounts"), true, "", preferences.show_icons, zh, targets, marker);
}

fn show(app: tauri::AppHandle, config: AppConfig, snapshot: Option<DashboardSnapshot>, usage: Option<modules::menu_bar_usage::MenuBarUsage>, status: Option<modules::auto_switch::Status>, reserve: u8, _anchor: Option<tauri::Rect>, ticket: u64) {
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
    let scope_name = match scope { crate::models::config::MenuBarQuotaScope::All => if zh { "Gemini 与 Claude/GPT" } else { "Gemini / Claude & GPT" }, crate::models::config::MenuBarQuotaScope::Gemini => if zh { "Gemini 系列" } else { "Gemini" }, crate::models::config::MenuBarQuotaScope::Other => if zh { "Claude 和 GPT 系列" } else { "Claude & GPT" } };
    let header = section(marker, 34.0);
    if preferences.show_icons { image(&header, NSApplication::sharedApplication(marker).applicationIconImage(), rect(20.0, 5.0, 24.0, 24.0), marker); }
    let title_x = if preferences.show_icons { 53.0 } else { 20.0 };
    label(&header, BRAND, title_x, 8.0, WIDTH - title_x - 20.0, 13.0, true, false, marker);
    custom_item(&menu, &header, BRAND, marker);
    menu.addItem(&NSMenuItem::separatorItem(marker));
    usage_section(&menu, usage.as_ref(), zh, marker);
    if preferences.show_aggregate {
      let heading = section(marker, 29.0);
      label(&heading, if zh { "剩余额度" } else { "Remaining quota" }, 20.0, 5.0, 140.0, 13.0, true, false, marker);
      let summary = label(&heading, &format!("{scope_name}  {}", if zh { "平均剩余" } else { "Mean remaining" }), 80.0, 7.0, WIDTH - 100.0, 11.0, false, true, marker);
      summary.setAlignment(objc2_app_kit::NSTextAlignment::Right);
      custom_item(&menu, &heading, "Overall quotas", marker);
    for period in (0..2).filter(|period| if *period == 0 { preferences.show_session } else { preferences.show_weekly }) {
        let (remaining, usable, _) = projection::aggregate(&windows, scope, period, reserve);
        let view = section(marker, 48.0);
        label(&view, if period == 0 { if zh { "5 小时" } else { "5 hours" } } else { if zh { "每周" } else { "Weekly" } }, 20.0, 5.0, 85.0, 13.0, true, false, marker);
        let stats = label(&view, &format!("{} {usable}/{}   {} {}", if zh { "可用账号" } else { "Available" }, accounts.len(), if zh { "剩余" } else { "Left" }, projection::percent(remaining)), 118.0, 7.0, WIDTH - 138.0, 11.0, false, true, marker);
        stats.setAlignment(objc2_app_kit::NSTextAlignment::Right);
        bar(&view, remaining, preferences, rect(20.0, 30.0, WIDTH - 40.0, 6.0), false, marker);
        custom_item(&menu, &view, if period == 0 { "5 hours" } else { "Weekly" }, marker);
    }
    menu.addItem(&NSMenuItem::separatorItem(marker));
    }
    let account_header = section(marker, 29.0);
    label(&account_header, if zh { "账号列表" } else { "Accounts" }, 20.0, 4.0, 160.0, 13.0, true, false, marker);
    let count = label(&account_header, &if zh { format!("{} 个账号", accounts.len()) } else { format!("{} accounts", accounts.len()) }, 210.0, 5.0, WIDTH - 230.0, 11.0, false, true, marker);
    count.setAlignment(objc2_app_kit::NSTextAlignment::Right);
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
        standard_item(&menu, &app, if zh { "取消待切换操作" } else { "Cancel pending switch" }, Action::Cancel(status.pending_id.unwrap()), !busy, "", preferences.show_icons, zh, &mut targets, marker);
    }
    standard_item(&menu, &app, if zh { "刷新全部额度" } else { "Refresh All Quotas" }, Action::Refresh, !busy, "r", preferences.show_icons, zh, &mut targets, marker);
    standard_item(&menu, &app, if zh { "用量看板" } else { "Usage Dashboard" }, Action::Page("dashboard"), true, "", preferences.show_icons, zh, &mut targets, marker);
    standard_item(&menu, &app, if zh { "管理账号" } else { "Manage Accounts" }, Action::Page("accounts"), true, "", preferences.show_icons, zh, &mut targets, marker);
    menu.addItem(&NSMenuItem::separatorItem(marker));
    standard_item(&menu, &app, if zh { "设置" } else { "Settings" }, Action::Page("settings"), true, ",", preferences.show_icons, zh, &mut targets, marker);
    standard_item(&menu, &app, "GitHub ↗", Action::Github, true, "", preferences.show_icons, zh, &mut targets, marker);
    standard_item(&menu, &app, if zh { "退出" } else { "Quit" }, Action::Quit, true, "q", preferences.show_icons, zh, &mut targets, marker);
    // Prebuild each detail page once. Only the selected page's items are
    // visible; the brand header stays in place throughout native tracking.
    let overview = menu.itemArray().iter().skip(2).collect();
    let mut detail_pages = Vec::new();
    for account in &accounts {
        let start = menu.itemArray().len();
        account_details(&menu, &app, account, preferences, zh, &mut targets, marker);
        let items: Vec<_> = menu.itemArray().iter().skip(start).collect();
        for item in &items { item.setHidden(true); }
        detail_pages.push((account.id.clone(), items));
    }
    PAGES.with(|pages| *pages.borrow_mut() = Some(MenuPages { overview, details: detail_pages }));
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
    PAGES.with(|pages| pages.borrow_mut().take());
    if GENERATION.load(Ordering::Acquire) == ticket { OPEN.store(false, Ordering::Release); }
    drop(targets);
}

pub fn toggle(app: &tauri::AppHandle, anchor: Option<tauri::Rect>) -> Result<(), String> {
    let ticket = GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    if OPEN.swap(true, Ordering::AcqRel) {
        OPEN.store(false, Ordering::Release);
        return app.run_on_main_thread(|| ACTIVE.with(|active| { if let Some(menu) = active.borrow().as_ref() { menu.cancelTrackingWithoutAnimation(); } })).map_err(|error| error.to_string());
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let (snapshot, usage) = tokio::join!(commands::get_menu_bar_snapshot(), tokio::time::timeout(std::time::Duration::from_secs(2), commands::get_menu_bar_usage()));
        let snapshot = snapshot.ok();
        let usage = usage.ok().and_then(Result::ok);
        if !OPEN.load(Ordering::Acquire) || GENERATION.load(Ordering::Acquire) != ticket { return; }
        let config = modules::load_app_config().unwrap_or_default();
        let status = modules::auto_switch::get_auto_switch_status(app.clone()).ok();
        let reserve = modules::auto_switch::get_auto_switch_config(app.clone()).map(|config| config.reserve_percentage).unwrap_or(10);
        let handle = app.clone();
        if handle.run_on_main_thread(move || show(app, config, snapshot, usage, status, reserve, anchor, ticket)).is_err() { OPEN.store(false, Ordering::Release); }
    });
    Ok(())
}
