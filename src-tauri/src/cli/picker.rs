use super::output::{AccountView, QuotaView, Snapshot};
use super::{CliError, HeadlessIntegration};
use std::io::{self, Read, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Zh,
    En,
}

impl Lang {
    pub fn current(root: &Path) -> Self {
        if let Ok(content) = std::fs::read_to_string(root.join("gui_config.json")) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(l) = v.get("language").and_then(|l| l.as_str()) {
                    if l.starts_with("zh") {
                        return Lang::Zh;
                    } else if l.starts_with("en") {
                        return Lang::En;
                    }
                }
            }
        }
        if let Some(loc) = sys_locale::get_locale() {
            if loc.starts_with("zh") {
                return Lang::Zh;
            }
        }
        Lang::En
    }
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::new();
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn display_width(s: &str) -> usize {
    let clean = strip_ansi(s);
    clean
        .chars()
        .map(|c| {
            let u = c as u32;
            if (0x4E00..=0x9FFF).contains(&u)
                || (0x3400..=0x4DBF).contains(&u)
                || (0x20000..=0x2A6DF).contains(&u)
                || (0xF900..=0xFAFF).contains(&u)
                || (0xFF01..=0xFF60).contains(&u)
                || (0xFFE0..=0xFFE6).contains(&u)
            {
                2
            } else {
                1
            }
        })
        .sum()
}

pub fn pad_right(s: &str, target_width: usize) -> String {
    let w = display_width(s);
    if w >= target_width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(target_width - w))
    }
}

pub fn get_terminal_width() -> usize {
    #[cfg(unix)]
    {
        unsafe {
            let mut ws: libc::winsize = std::mem::zeroed();
            if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) == 0 && ws.ws_col > 0 {
                return ws.ws_col as usize;
            }
        }
    }
    std::env::var("COLUMNS")
        .ok()
        .and_then(|c| c.parse::<usize>().ok())
        .unwrap_or(80)
}

pub fn truncate_display_width(s: &str, max_w: usize) -> String {
    let clean = strip_ansi(s);
    if display_width(&clean) <= max_w {
        return s.to_string();
    }
    let budget = if max_w > 1 { max_w - 1 } else { max_w };
    let mut out = String::new();
    let mut cur_w = 0;
    for c in clean.chars() {
        let cw = display_width(&c.to_string());
        if cur_w + cw > budget {
            break;
        }
        out.push(c);
        cur_w += cw;
    }
    if max_w > 1 {
        out.push('…');
    }
    out
}

pub fn pad_left(s: &str, target_width: usize) -> String {
    let w = display_width(s);
    if w >= target_width {
        s.to_string()
    } else {
        format!("{}{}", " ".repeat(target_width - w), s)
    }
}

pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    align_right: Vec<bool>,
}

impl Table {
    pub fn new(headers: Vec<&str>) -> Self {
        let count = headers.len();
        Self {
            headers: headers.into_iter().map(String::from).collect(),
            rows: Vec::new(),
            align_right: vec![false; count],
        }
    }

    pub fn set_align_right(&mut self, col: usize, right: bool) {
        if col < self.align_right.len() {
            self.align_right[col] = right;
        }
    }

    pub fn add_row(&mut self, row: Vec<String>) {
        self.rows.push(row);
    }

    pub fn render(&self) -> String {
        let term_w = get_terminal_width();
        let max_w = term_w.saturating_sub(2).min(78).max(40);
        self.render_with_max_width(max_w)
    }

    pub fn render_with_max_width(&self, max_width: usize) -> String {
        let num_cols = self.headers.len();
        if num_cols == 0 {
            return String::new();
        }
        let mut col_widths = vec![0; num_cols];

        for (i, h) in self.headers.iter().enumerate() {
            col_widths[i] = col_widths[i].max(display_width(h));
        }

        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < num_cols {
                    col_widths[i] = col_widths[i].max(display_width(cell));
                }
            }
        }

        let border_overhead = num_cols * 3 + 1;
        let mut total_content_width: usize = col_widths.iter().sum();
        let total_width = total_content_width + border_overhead;

        if total_width > max_width && max_width > border_overhead {
            let max_content_width = max_width - border_overhead;
            while total_content_width > max_content_width {
                let (widest_idx, &widest_w) = col_widths
                    .iter()
                    .enumerate()
                    .max_by_key(|&(_, w)| *w)
                    .unwrap();
                if widest_w <= 4 {
                    break;
                }
                col_widths[widest_idx] -= 1;
                total_content_width -= 1;
            }
        }

        let mut out = String::new();

        // Top border: ┌───┬───┐
        out.push_str("\r┌");
        for (i, w) in col_widths.iter().enumerate() {
            out.push_str(&"─".repeat(*w + 2));
            if i + 1 < num_cols {
                out.push('┬');
            }
        }
        out.push_str("┐\n");

        // Header: │ Title │ ... │
        out.push_str("\r│");
        for (i, h) in self.headers.iter().enumerate() {
            out.push(' ');
            let w = col_widths[i];
            let clean = strip_ansi(h);
            let truncated = if display_width(&clean) > w {
                truncate_display_width(&clean, w)
            } else {
                h.clone()
            };
            let cell = if self.align_right.get(i).copied().unwrap_or(false) {
                pad_left(&truncated, w)
            } else {
                pad_right(&truncated, w)
            };
            out.push_str(&format!("\x1b[1m{}\x1b[0m", cell));
            out.push(' ');
            out.push('│');
        }
        out.push('\n');

        // Header separator: ├───┼───┤
        out.push_str("\r├");
        for (i, w) in col_widths.iter().enumerate() {
            out.push_str(&"─".repeat(*w + 2));
            if i + 1 < num_cols {
                out.push('┼');
            }
        }
        out.push_str("┤\n");

        // Data rows
        for row in &self.rows {
            out.push_str("\r│");
            for (i, cell) in row.iter().enumerate() {
                out.push(' ');
                let clean = strip_ansi(cell);
                let w = col_widths[i];
                let formatted = if display_width(&clean) > w {
                    let truncated = truncate_display_width(&clean, w);
                    if self.align_right.get(i).copied().unwrap_or(false) {
                        pad_left(&truncated, w)
                    } else {
                        pad_right(&truncated, w)
                    }
                } else {
                    if self.align_right.get(i).copied().unwrap_or(false) {
                        pad_left(cell, w)
                    } else {
                        pad_right(cell, w)
                    }
                };
                out.push_str(&formatted);
                out.push(' ');
                out.push('│');
            }
            out.push('\n');
        }

        // Bottom border: └───┴───┘
        out.push_str("\r└");
        for (i, w) in col_widths.iter().enumerate() {
            out.push_str(&"─".repeat(*w + 2));
            if i + 1 < num_cols {
                out.push('┴');
            }
        }
        out.push_str("┘\n");

        out
    }
}

pub(crate) fn quota_brief(quota: Option<&QuotaView>, lang: Lang) -> String {
    if let Some(q) = quota {
        if q.is_forbidden {
            return match lang {
                Lang::Zh => " (额度受限)".into(),
                Lang::En => " (Forbidden)".into(),
            };
        }
        let mut parts = Vec::new();
        if let Some(groups) = &q.quota_groups {
            for g in groups {
                if let Some(b) = g.buckets.first() {
                    let pct = (b.remaining_fraction * 100.0).round() as i32;
                    let short_name = if g.display_name.contains("Gemini") {
                        "Gemini"
                    } else if g.display_name.contains("Claude") || g.display_name.contains("GPT") {
                        "Claude/GPT"
                    } else {
                        &g.display_name
                    };
                    parts.push(format!("{}: {}%", short_name, pct));
                }
            }
        } else {
            for m in q.models.iter().take(2) {
                parts.push(format!("{}: {}%", m.name, m.percentage));
            }
        }
        if !parts.is_empty() {
            return format!(" ({})", parts.join(" | "));
        }
    }
    String::new()
}

pub(crate) fn format_countdown_compact(reset_time_str: &str, lang: Lang) -> String {
    if reset_time_str.is_empty() {
        return String::new();
    }
    if let Ok(reset_dt) = chrono::DateTime::parse_from_rfc3339(reset_time_str) {
        let now = chrono::Utc::now();
        let diff = reset_dt.signed_duration_since(now.with_timezone(&reset_dt.timezone()));
        if diff.num_seconds() <= 0 {
            match lang {
                Lang::Zh => "已重置".to_string(),
                Lang::En => "Ready".to_string(),
            }
        } else {
            let hours = diff.num_hours();
            let mins = diff.num_minutes() % 60;
            if hours > 0 {
                format!("{}h", hours)
            } else {
                format!("{}m", mins.max(1))
            }
        }
    } else {
        String::new()
    }
}

pub(crate) fn format_countdown(reset_time_str: &str, lang: Lang) -> String {
    if reset_time_str.is_empty() {
        return String::new();
    }
    if let Ok(reset_dt) = chrono::DateTime::parse_from_rfc3339(reset_time_str) {
        let now = chrono::Utc::now();
        let diff = reset_dt.signed_duration_since(now.with_timezone(&reset_dt.timezone()));
        if diff.num_seconds() <= 0 {
            match lang {
                Lang::Zh => "已重置".to_string(),
                Lang::En => "Reset".to_string(),
            }
        } else {
            let hours = diff.num_hours();
            let mins = diff.num_minutes() % 60;
            match lang {
                Lang::Zh => {
                    if hours > 0 {
                        format!("{}小时{}分", hours, mins)
                    } else {
                        format!("{}分", mins.max(1))
                    }
                }
                Lang::En => {
                    if hours > 0 {
                        format!("{}h {}m", hours, mins)
                    } else {
                        format!("{}m", mins.max(1))
                    }
                }
            }
        }
    } else {
        reset_time_str.to_string()
    }
}

pub(crate) fn format_number(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut result = String::new();
    let len = bytes.len();
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(b as char);
    }
    result
}

pub(crate) fn progress_bar(percentage: i32, width: usize) -> String {
    let clamped = percentage.clamp(0, 100);
    let mut filled = ((clamped as f64 / 100.0) * width as f64).round() as usize;
    if clamped > 0 && filled == 0 {
        filled = 1;
    }
    if clamped >= 100 {
        filled = width;
    }
    let empty = width.saturating_sub(filled);

    let color = if clamped > 50 {
        "\x1b[32m" // green
    } else if clamped > 20 {
        "\x1b[33m" // yellow
    } else {
        "\x1b[31m" // red
    };

    let filled_part = if filled > 0 {
        format!("{}{}\x1b[0m", color, "▰".repeat(filled))
    } else {
        String::new()
    };

    let empty_part = if empty > 0 {
        format!("\x1b[38;2;180;175;165m{}\x1b[0m", "▱".repeat(empty))
    } else {
        String::new()
    };

    format!("{}{}", filled_part, empty_part)
}

pub fn estimate_model_cost(input: u64, output: u64, cached: u64, model: &str) -> f64 {
    let m = model.to_lowercase();
    let (in_p, out_p, cache_p) = if m.contains("sonnet") || m.contains("claude") || m.contains("opus") {
        (3.0, 15.0, 0.3)
    } else if m.contains("flash") && m.contains("image") {
        (0.5, 60.0, 0.0)
    } else if m.contains("3.8-flash") || m.contains("3.8") {
        (0.75, 3.75, 0.075)
    } else if m.contains("3.1-pro") || m.contains("pro") {
        (2.0, 12.0, 0.2)
    } else if m.contains("3.5-flash") {
        (1.5, 9.0, 0.15)
    } else if m.contains("flash-lite") {
        (0.1, 0.4, 0.01)
    } else if m.contains("flash") {
        (0.5, 3.0, 0.05)
    } else {
        (0.75, 3.75, 0.075)
    };
    (input as f64 * in_p + output as f64 * out_p + cached as f64 * cache_p) / 1_000_000.0
}

pub fn format_cost(usd: f64) -> String {
    if usd >= 1000.0 {
        format!("${:.2}k", usd / 1000.0)
    } else if usd >= 100.0 {
        format!("${:.1}", usd)
    } else if usd >= 1.0 {
        format!("${:.2}", usd)
    } else if usd > 0.001 {
        format!("${:.3}", usd)
    } else {
        "$0.00".to_string()
    }
}

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
}

#[cfg(unix)]
struct RawTerminal {
    orig: libc::termios,
}

#[cfg(unix)]
impl RawTerminal {
    fn enter() -> Option<Self> {
        unsafe {
            if libc::isatty(libc::STDIN_FILENO) != 1 || libc::isatty(libc::STDOUT_FILENO) != 1 {
                return None;
            }
            let mut orig = std::mem::zeroed();
            if libc::tcgetattr(libc::STDIN_FILENO, &mut orig) != 0 {
                return None;
            }
            let mut raw = orig;
            libc::cfmakeraw(&mut raw);
            raw.c_oflag |= libc::OPOST | libc::ONLCR;
            raw.c_cc[libc::VMIN] = 0;
            raw.c_cc[libc::VTIME] = 1; // 100ms
            if libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw) != 0 {
                return None;
            }
            Some(Self { orig })
        }
    }
}

#[cfg(unix)]
impl Drop for RawTerminal {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &self.orig);
            print!("\x1b[?25h"); // Show cursor
            let _ = io::stdout().flush();
        }
    }
}

enum KeyAction {
    Up,
    Down,
    Enter,
    Cancel,
    SelectIndex(usize),
    Char(char),
    None,
}

#[cfg(unix)]
fn read_key_action() -> KeyAction {
    let mut byte = [0u8; 1];
    let mut stdin = io::stdin();

    loop {
        match stdin.read(&mut byte) {
            Ok(1) => break,
            Ok(0) => continue,
            _ => return KeyAction::Cancel,
        }
    }

    match byte[0] {
        b'\r' | b'\n' => KeyAction::Enter,
        b'\x03' | b'q' | b'Q' => KeyAction::Cancel,
        b'k' | b'K' => KeyAction::Up,
        b'j' | b'J' => KeyAction::Down,
        b'0' => KeyAction::Char('0'),
        b'1'..=b'9' => KeyAction::SelectIndex((byte[0] - b'1') as usize),
        b'\x1b' => {
            let mut seq = [0u8; 2];
            match stdin.read(&mut seq[0..1]) {
                Ok(1) if seq[0] == b'[' => match stdin.read(&mut seq[1..2]) {
                    Ok(1) => match seq[1] {
                        b'A' => KeyAction::Up,
                        b'B' => KeyAction::Down,
                        _ => KeyAction::None,
                    },
                    _ => KeyAction::Cancel,
                },
                _ => KeyAction::Cancel,
            }
        }
        b => KeyAction::Char(b as char),
    }
}

fn prompt_line(prompt: &str) -> String {
    print!("{}", prompt);
    let _ = io::stdout().flush();
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
    line.trim().to_string()
}

fn wait_for_key(lang: Lang) {
    let msg = match lang {
        Lang::Zh => "按任意键继续...",
        Lang::En => "Press any key to continue...",
    };
    print!("\n\x1b[2m{}\x1b[0m", msg);
    let _ = io::stdout().flush();
    #[cfg(unix)]
    {
        if let Some(_raw) = RawTerminal::enter() {
            let _ = read_key_action();
            print!("\x1b[2K\r");
            let _ = io::stdout().flush();
            return;
        }
    }
    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
}

pub fn select_menu_interactive(title: &str, items: &[&str], initial: usize, lang: Lang) -> Option<usize> {
    if items.is_empty() {
        return None;
    }

    #[cfg(unix)]
    {
        if let Some(_raw) = RawTerminal::enter() {
            let mut selected = initial.min(items.len() - 1);
            let mut stdout = io::stdout();

            print!("\x1b[?25l");
            let _ = stdout.flush();

            let hint = match lang {
                Lang::Zh => "(↑/↓ 移动  |  回车确认  |  数字键选择  |  0/Q 退出)",
                Lang::En => "(↑/↓ Navigate  |  Enter Select  |  Numbers  |  0/Q Quit)",
            };

            let render = |sel: usize, first: bool| {
                let mut out = io::stdout();
                if !first {
                    print!("\x1b[{}A", items.len() + 1);
                }
                println!("\x1b[2K\r\x1b[1m{}\x1b[0m \x1b[90m{}\x1b[0m", title, hint);
                for (i, item) in items.iter().enumerate() {
                    if i == sel {
                        println!("\x1b[2K\r  \x1b[1;36m➤\x1b[0m \x1b[1m{}\x1b[0m", item);
                    } else {
                        println!("\x1b[2K\r    {}", item);
                    }
                }
                let _ = out.flush();
            };

            render(selected, true);

            loop {
                match read_key_action() {
                    KeyAction::Up => {
                        selected = if selected > 0 {
                            selected - 1
                        } else {
                            items.len() - 1
                        };
                        render(selected, false);
                    }
                    KeyAction::Down => {
                        selected = if selected + 1 < items.len() {
                            selected + 1
                        } else {
                            0
                        };
                        render(selected, false);
                    }
                    KeyAction::SelectIndex(idx) => {
                        let digit = (idx + 1).to_string();
                        let prefix = format!("{}.", digit);
                        for (i, item) in items.iter().enumerate() {
                            if item.contains(&prefix) || item.starts_with(&digit) {
                                print!("\x1b[?25h");
                                let _ = stdout.flush();
                                return Some(i);
                            }
                        }
                        if idx < items.len() {
                            print!("\x1b[?25h");
                            let _ = stdout.flush();
                            return Some(idx);
                        }
                    }
                    KeyAction::Char('0') => {
                        for (i, item) in items.iter().enumerate() {
                            if item.contains("0.") || item.starts_with('0') {
                                print!("\x1b[?25h");
                                let _ = stdout.flush();
                                return Some(i);
                            }
                        }
                        print!("\x1b[?25h");
                        let _ = stdout.flush();
                        return None;
                    }
                    KeyAction::Char(ch) => {
                        let upper = ch.to_ascii_uppercase();
                        let prefix = format!("{}.", upper);
                        for (i, item) in items.iter().enumerate() {
                            if item.contains(&prefix) {
                                print!("\x1b[?25h");
                                let _ = stdout.flush();
                                return Some(i);
                            }
                        }
                    }
                    KeyAction::Enter => {
                        print!("\x1b[?25h");
                        let _ = stdout.flush();
                        return Some(selected);
                    }
                    KeyAction::Cancel => {
                        print!("\x1b[?25h");
                        let _ = stdout.flush();
                        return None;
                    }
                    KeyAction::None => {}
                }
            }
        }
    }

    println!("{}", title);
    for item in items.iter() {
        println!("  {}", item);
    }
    let prompt_msg = match lang {
        Lang::Zh => format!("请输入选项序号 [1-{}]: ", items.len()),
        Lang::En => format!("Select option [1-{}]: ", items.len()),
    };
    print!("{}", prompt_msg);
    let _ = io::stdout().flush();
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        let trimmed = input.trim();
        if let Ok(num) = trimmed.parse::<usize>() {
            if num >= 1 && num <= items.len() {
                return Some(num - 1);
            }
        }
    }
    None
}

fn build_accounts_table(accounts: &[AccountView], lang: Lang, selected_idx: Option<usize>) -> Table {
    let headers = match lang {
        Lang::Zh => vec!["#", "账号 (备注)", "Gemini", "Claude/GPT", "状态"],
        Lang::En => vec!["#", "Account (Label)", "Gemini", "Claude/GPT", "Status"],
    };
    let mut table = Table::new(headers);

    for (i, acc) in accounts.iter().enumerate() {
        let is_selected = selected_idx == Some(i);
        let prefix = if is_selected {
            format!("➤{}", i + 1)
        } else {
            format!(" {}", i + 1)
        };

        let label = match &acc.custom_label {
            Some(l) if !l.trim().is_empty() => format!(" ({})", l.trim()),
            _ => String::new(),
        };
        let cur = if acc.is_current { " *" } else { "" };
        let display_name = format!("{}{}{}", acc.email, label, cur);

        let mut gemini_quota = match lang {
            Lang::Zh => "暂无".to_string(),
            Lang::En => "None".to_string(),
        };
        let mut claude_quota = match lang {
            Lang::Zh => "暂无".to_string(),
            Lang::En => "None".to_string(),
        };

        if let Some(q) = &acc.quota {
            if q.is_forbidden {
                gemini_quota = match lang {
                    Lang::Zh => "受限".into(),
                    Lang::En => "Forbidden".into(),
                };
                claude_quota = gemini_quota.clone();
            } else if let Some(groups) = &q.quota_groups {
                for g in groups {
                    let weekly_b = g.buckets.iter().find(|b| b.bucket_id.contains("week") || b.window.contains("week"));
                    let five_h_b = g.buckets.iter().find(|b| b.bucket_id.contains("5h") || b.window.contains("5h"));

                    let (chosen_b, is_weekly) = match (weekly_b, five_h_b) {
                        (Some(wb), Some(fb)) => {
                            let w_pct = (wb.remaining_fraction * 100.0).round() as i32;
                            let f_pct = (fb.remaining_fraction * 100.0).round() as i32;
                            if w_pct < 100 && f_pct < 100 {
                                if f_pct < w_pct {
                                    (fb, false)
                                } else {
                                    (wb, true)
                                }
                            } else if w_pct < 100 {
                                (wb, true)
                            } else if f_pct < 100 {
                                (fb, false)
                            } else {
                                (wb, true)
                            }
                        }
                        (Some(wb), None) => (wb, true),
                        (None, Some(fb)) => (fb, false),
                        _ => match g.buckets.first() {
                            Some(b) => (b, b.bucket_id.contains("week")),
                            None => continue,
                        },
                    };

                    let pct = (chosen_b.remaining_fraction * 100.0).round() as i32;
                    let cd = format_countdown_compact(&chosen_b.reset_time, lang);
                    let tag = if is_weekly {
                        match lang {
                            Lang::Zh => "周",
                            Lang::En => "Wk",
                        }
                    } else {
                        "5h"
                    };

                    let text = if cd.is_empty() || pct >= 100 {
                        format!("{}%", pct)
                    } else {
                        format!("{}% ({}: {})", pct, tag, cd)
                    };
                    if g.display_name.contains("Gemini") {
                        gemini_quota = text;
                    } else if g.display_name.contains("Claude") || g.display_name.contains("GPT") {
                        claude_quota = text;
                    }
                }
            } else {
                for m in &q.models {
                    if m.name.contains("gemini") {
                        gemini_quota = format!("{}%", m.percentage);
                    } else if m.name.contains("claude") {
                        claude_quota = format!("{}%", m.percentage);
                    }
                }
            }
        }

        let status = if acc.disabled {
            match lang {
                Lang::Zh => "\x1b[31m已禁用\x1b[0m",
                Lang::En => "\x1b[31mDisabled\x1b[0m",
            }
        } else if acc.validation_blocked {
            match lang {
                Lang::Zh => "\x1b[33m需验证\x1b[0m",
                Lang::En => "\x1b[33mVerify\x1b[0m",
            }
        } else if acc.is_current {
            match lang {
                Lang::Zh => "\x1b[32m当前生效\x1b[0m",
                Lang::En => "\x1b[32mActive\x1b[0m",
            }
        } else {
            match lang {
                Lang::Zh => "正常",
                Lang::En => "Normal",
            }
        };

        let row_display_name = if is_selected {
            format!("\x1b[1;36m{}\x1b[0m", display_name)
        } else {
            display_name
        };

        table.add_row(vec![
            prefix,
            row_display_name,
            gemini_quota,
            claude_quota,
            status.to_string(),
        ]);
    }
    table
}

pub fn select_account_interactive<'a>(
    accounts: &'a [AccountView],
    lang: Lang,
) -> Option<&'a AccountView> {
    if accounts.is_empty() {
        return None;
    }

    #[cfg(unix)]
    {
        if let Some(_raw) = RawTerminal::enter() {
            let mut selected = accounts.iter().position(|a| a.is_current).unwrap_or(0);
            let mut stdout = io::stdout();

            print!("\x1b[?25l");
            let _ = stdout.flush();

            let prompt_text = match lang {
                Lang::Zh => "? 请选择账号 (↑/↓ 移动  |  回车确认  |  数字键直选  |  0/Q 取消):",
                Lang::En => "? Select account (↑/↓ Navigate  |  Enter Select  |  Numbers  |  0/Q Cancel):",
            };

            let render = |sel: usize, initial: bool| {
                let mut out = io::stdout();
                let table_str = build_accounts_table(accounts, lang, Some(sel)).render();
                let table_lines = table_str.lines().count();
                let total_lines = table_lines + 2;

                if !initial {
                    print!("\x1b[{}A", total_lines);
                }

                for line in table_str.lines() {
                    println!("\x1b[2K\r{}", line);
                }
                println!("\x1b[2K\r\x1b[1m{}\x1b[0m", prompt_text);
                print!("\x1b[2K\r");
                let _ = out.flush();
            };

            render(selected, true);

            loop {
                match read_key_action() {
                    KeyAction::Up => {
                        selected = if selected > 0 {
                            selected - 1
                        } else {
                            accounts.len() - 1
                        };
                        render(selected, false);
                    }
                    KeyAction::Down => {
                        selected = if selected + 1 < accounts.len() {
                            selected + 1
                        } else {
                            0
                        };
                        render(selected, false);
                    }
                    KeyAction::SelectIndex(idx) => {
                        if idx < accounts.len() {
                            print!("\x1b[?25h");
                            let _ = stdout.flush();
                            return Some(&accounts[idx]);
                        }
                    }
                    KeyAction::Char('0') | KeyAction::Cancel => {
                        print!("\x1b[?25h");
                        let _ = stdout.flush();
                        return None;
                    }
                    KeyAction::Enter => {
                        print!("\x1b[?25h");
                        let _ = stdout.flush();
                        return Some(&accounts[selected]);
                    }
                    _ => {}
                }
            }
        }
    }

    let table_str = build_accounts_table(accounts, lang, None).render();
    print!("{}", table_str);
    let prompt_msg = match lang {
        Lang::Zh => format!("请输入账号序号 [1-{}]: ", accounts.len()),
        Lang::En => format!("Select account number [1-{}]: ", accounts.len()),
    };
    print!("{}", prompt_msg);
    let _ = io::stdout().flush();
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        let trimmed = input.trim();
        if let Ok(num) = trimmed.parse::<usize>() {
            if num >= 1 && num <= accounts.len() {
                return Some(&accounts[num - 1]);
            }
        }
    }
    None
}


pub fn format_token_stats_human(
    summary: &crate::modules::native_token_stats::LocalTokenUsageSummary,
    lang: Lang,
) -> String {
    let mut out = String::new();

    let headers = match lang {
        Lang::Zh => vec!["周期", "总计 Token", "输入", "输出", "缓存率", "预期费用"],
        Lang::En => vec!["Period", "Total Tokens", "Input", "Output", "Cache Hit", "Est. Cost"],
    };

    let mut table = Table::new(headers);
    for col in 1..=5 {
        table.set_align_right(col, true);
    }

    let periods = match lang {
        Lang::Zh => [
            ("今日", &summary.today),
            ("昨日", &summary.yesterday),
            ("近 3 天", &summary.last_3_days),
            ("近 7 天", &summary.last_7_days),
            ("近 30 天", &summary.last_30_days),
        ],
        Lang::En => [
            ("Today", &summary.today),
            ("Yesterday", &summary.yesterday),
            ("Last 3 Days", &summary.last_3_days),
            ("Last 7 Days", &summary.last_7_days),
            ("Last 30 Days", &summary.last_30_days),
        ],
    };

    for (i, (name, row)) in periods.iter().enumerate() {
        let total_in = row.input_tokens + row.cached_tokens;
        let hit_rate = if total_in > 0 {
            format!("{:.1}%", (row.cached_tokens as f64 / total_in as f64) * 100.0)
        } else {
            "0.0%".into()
        };

        let cost = if i == 0 && !summary.by_model_today.is_empty() {
            summary.by_model_today.iter().map(|m| {
                estimate_model_cost(m.input_tokens, m.output_tokens, m.cached_tokens, &m.model)
            }).sum()
        } else {
            estimate_model_cost(row.input_tokens, row.output_tokens, row.cached_tokens, "gemini-3.8-flash")
        };

        table.add_row(vec![
            name.to_string(),
            format_number(row.total_tokens),
            format_number(row.input_tokens),
            format_number(row.output_tokens),
            hit_rate,
            format_cost(cost),
        ]);
    }

    out.push_str(&table.render());

    if !summary.by_model_today.is_empty() {
        let title = match lang {
            Lang::Zh => "\n【今日各模型用量明细】\n",
            Lang::En => "\n[Model Breakdown: Today]\n",
        };
        out.push_str(title);

        let m_headers = match lang {
            Lang::Zh => vec!["模型", "总计 Token", "输入", "输出", "缓存", "预期费用"],
            Lang::En => vec!["Model", "Total Tokens", "Input", "Output", "Cached", "Est. Cost"],
        };
        let mut m_table = Table::new(m_headers);
        for col in 1..=5 {
            m_table.set_align_right(col, true);
        }
        for m in &summary.by_model_today {
            let cost = estimate_model_cost(m.input_tokens, m.output_tokens, m.cached_tokens, &m.model);
            m_table.add_row(vec![
                m.model.clone(),
                format_number(m.total_tokens),
                format_number(m.input_tokens),
                format_number(m.output_tokens),
                format_number(m.cached_tokens),
                format_cost(cost),
            ]);
        }
        out.push_str(&m_table.render());
    }

    if !summary.by_model.is_empty() {
        let title = match lang {
            Lang::Zh => "\n【近 30 天主要模型用量排行】\n",
            Lang::En => "\n[Top Models: Last 30 Days]\n",
        };
        out.push_str(title);

        let m_headers = match lang {
            Lang::Zh => vec!["模型", "总计 Token", "请求次数", "预期费用"],
            Lang::En => vec!["Model", "Total Tokens", "Requests", "Est. Cost"],
        };
        let mut m_table = Table::new(m_headers);
        m_table.set_align_right(1, true);
        m_table.set_align_right(2, true);
        m_table.set_align_right(3, true);
        for m in summary.by_model.iter().take(5) {
            let cost = estimate_model_cost(m.input_tokens, m.output_tokens, m.cached_tokens, &m.model);
            m_table.add_row(vec![
                m.model.clone(),
                format_number(m.total_tokens),
                format_number(m.request_count),
                format_cost(cost),
            ]);
        }
        out.push_str(&m_table.render());
    }

    let footer = match lang {
        Lang::Zh => format!(
            "\n数据来源: 扫描 {} 个本地 SQLite 数据库 | 累计对话记录: {} 条\n",
            summary.databases_scanned, summary.generations_scanned
        ),
        Lang::En => format!(
            "\nSource: Scanned {} local SQLite databases | Total records: {}\n",
            summary.databases_scanned, summary.generations_scanned
        ),
    };
    out.push_str(&footer);

    out
}

pub fn run_interactive_dashboard(root: &Path) -> Result<(), CliError> {
    loop {
        let lang = Lang::current(root);
        let snapshot = Snapshot::read(root).unwrap_or(Snapshot {
            accounts: vec![],
            current_target: None,
        });

        // Clear screen and show minimal, professional header
        print!("\x1b[2J\x1b[H");
        match lang {
            Lang::Zh => println!(
                "\x1b[1mAntigravity 账号管理\x1b[0m · \x1b[36magy-switch v{}\x1b[0m",
                env!("CARGO_PKG_VERSION")
            ),
            Lang::En => println!(
                "\x1b[1mAntigravity Tools Lite\x1b[0m · \x1b[36magy-switch v{}\x1b[0m",
                env!("CARGO_PKG_VERSION")
            ),
        }

        if let Ok(curr) = snapshot.current() {
            let target = snapshot.current_target.as_deref().unwrap_or("app");
            let label = match &curr.custom_label {
                Some(l) if !l.trim().is_empty() => format!(" ({})", l),
                _ => String::new(),
            };
            let qb = quota_brief(curr.quota.as_ref(), lang);
            let quota_part = if qb.is_empty() {
                String::new()
            } else {
                format!("  | {}", qb.trim_start_matches(" (").trim_end_matches(')'))
            };
            match lang {
                Lang::Zh => println!(
                    "\x1b[1m当前生效:\x1b[0m \x1b[1;32m{}{}\x1b[0m [{}] {}\n",
                    curr.email, label, target, quota_part
                ),
                Lang::En => println!(
                    "\x1b[1mActive:\x1b[0m \x1b[1;32m{}{}\x1b[0m [{}] {}\n",
                    curr.email, label, target, quota_part
                ),
            }
        } else {
            match lang {
                Lang::Zh => println!("\x1b[1m当前生效:\x1b[0m \x1b[33m未设置 / 暂无账号\x1b[0m\n"),
                Lang::En => println!("\x1b[1mActive:\x1b[0m \x1b[33mNone / No accounts\x1b[0m\n"),
            }
        }

        let (title, menu_items) = match lang {
            Lang::Zh => (
                "选择功能:",
                vec![
                    "1. 切换账号      选择生效账号与同步目标 (桌面应用 / 独立环境)",
                    "2. 配额详情      查看各模型配额余量与重置倒计时",
                    "3. 用量统计      本地 Token 消耗、预期费用与模型分布",
                    "4. 刷新配额      联网同步 Google API 最新额度",
                    "5. 添加账号      通过 Google OAuth 授权绑定新账号",
                    "6. 账号管理      修改备注标签、切换启用状态或删除账号",
                    "7. 环境状态      关联应用与本地存储状态",
                    "0. 退出控制台    退出当前工具",
                ],
            ),
            Lang::En => (
                "Select Command:",
                vec![
                    "1. Switch        Switch active account and session target (App / IDE)",
                    "2. Quotas        View model quotas and reset countdowns",
                    "3. Statistics    Inspect local token usage, estimated cost & models",
                    "4. Refresh       Fetch live quotas from Google API",
                    "5. Add Account   Authorize new Google account via OAuth",
                    "6. Manage        Edit labels, toggle status, or remove accounts",
                    "7. Status        Inspect linked applications and storage",
                    "0. Exit          Quit agy-switch",
                ],
            ),
        };

        let choice = select_menu_interactive(title, &menu_items, 0, lang);

        match choice {
            Some(0) => show_account_switcher(root, &snapshot, lang),
            Some(1) => show_quota_details(&snapshot, lang),
            Some(2) => show_token_statistics(lang),
            Some(3) => show_refresh_quotas(&snapshot, lang),
            Some(4) => show_add_account(lang),
            Some(5) => show_manage_accounts(&snapshot, lang),
            Some(6) => show_system_status(&snapshot, root, lang),
            Some(7) | None => {
                let exit_msg = match lang {
                    Lang::Zh => "\n已退出 agy-switch 控制台。\n",
                    Lang::En => "\nExited agy-switch.\n",
                };
                println!("{}", exit_msg);
                break;
            }
            _ => {}
        }
    }
    Ok(())
}

fn show_account_switcher(_root: &Path, snapshot: &Snapshot, lang: Lang) {
    if snapshot.accounts.is_empty() {
        let msg = match lang {
            Lang::Zh => "\n\x1b[33m暂无已保存账号，请使用 [5] 添加 Google 账号。\x1b[0m",
            Lang::En => "\n\x1b[33mNo saved accounts. Use [5] to add a Google account.\x1b[0m",
        };
        println!("{}", msg);
        wait_for_key(lang);
        return;
    }

    print!("\x1b[2J\x1b[H");
    let header = match lang {
        Lang::Zh => "切换当前生效账号",
        Lang::En => "Switch Active Account",
    };
    println!("\x1b[1m{}\x1b[0m\n", header);

    let selected = match select_account_interactive(&snapshot.accounts, lang) {
        Some(acc) => acc,
        None => return,
    };

    print!("\x1b[2J\x1b[H");
    let target_header = match lang {
        Lang::Zh => format!("已选择账号: \x1b[1;32m{}\x1b[0m\n", selected.email),
        Lang::En => format!("Selected account: \x1b[1;32m{}\x1b[0m\n", selected.email),
    };
    println!("{}", target_header);

    let (title, target_items) = match lang {
        Lang::Zh => (
            "选择生效目标:",
            vec![
                "1. AntiGravity 桌面应用与命令行 (同步生效)",
                "2. AntiGravity 独立环境 (专属通道)",
                "0. 取消并返回",
            ],
        ),
        Lang::En => (
            "Select Target:",
            vec![
                "1. Desktop App and CLI (Default sync)",
                "2. Dedicated IDE Channel",
                "0. Cancel",
            ],
        ),
    };

    let target_choice = select_menu_interactive(title, &target_items, 0, lang);

    let target_ide = match target_choice {
        Some(0) => None,
        Some(1) => Some("ide"),
        _ => return,
    };

    let wait_msg = match lang {
        Lang::Zh => "\n正在执行账号切换并同步会话...",
        Lang::En => "\nSwitching account and synchronizing session...",
    };
    println!("{}", wait_msg);

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let err_msg = match lang {
                Lang::Zh => format!("\x1b[31m启动异步运行时失败: {}\x1b[0m", e),
                Lang::En => format!("\x1b[31mFailed to start async runtime: {}\x1b[0m", e),
            };
            println!("{}", err_msg);
            wait_for_key(lang);
            return;
        }
    };

    match runtime.block_on(crate::modules::account::switch_account(
        &selected.id,
        target_ide,
        &HeadlessIntegration,
    )) {
        Ok(_) => {
            let succ_msg = match lang {
                Lang::Zh => format!(
                    "\x1b[1;32m✓ 账号切换成功: {}\x1b[0m (目标: {})\n提示: 请在终端开启新的 agy 命令以使用最新会话。\n",
                    selected.email,
                    target_ide.unwrap_or("app")
                ),
                Lang::En => format!(
                    "\x1b[1;32m✓ Successfully switched to: {}\x1b[0m (target: {})\nHint: Start a new agy command to use the updated session.\n",
                    selected.email,
                    target_ide.unwrap_or("app")
                ),
            };
            println!("{}", succ_msg);

        }
        Err(err) => {
            let fail_msg = match lang {
                Lang::Zh => format!("\x1b[31m账号切换失败: {}\x1b[0m", err),
                Lang::En => format!("\x1b[31mAccount switch failed: {}\x1b[0m", err),
            };
            println!("{}", fail_msg);
        }
    }
    wait_for_key(lang);
}

fn show_quota_details(snapshot: &Snapshot, lang: Lang) {
    if snapshot.accounts.is_empty() {
        let msg = match lang {
            Lang::Zh => "\n\x1b[33m暂无已保存账号。\x1b[0m",
            Lang::En => "\n\x1b[33mNo saved accounts.\x1b[0m",
        };
        println!("{}", msg);
        wait_for_key(lang);
        return;
    }

    loop {
        print!("\x1b[2J\x1b[H");
        let header = match lang {
            Lang::Zh => "全部账号配额总览",
            Lang::En => "All Accounts Quota Overview",
        };
        println!("\x1b[1m{}\x1b[0m\n", header);

        print!("{}", build_accounts_table(&snapshot.accounts, lang, None).render());

        let notes = match lang {
            Lang::Zh => [
                "* 标注为当前生效账号",
                "说明: 括号内标注重置时间，(周: 98h) 为7天周配额重置倒计时，(5h: 4h) 为5小时滚动窗口倒计时",
            ],
            Lang::En => [
                "* indicates active account",
                "Note: In parentheses: (Wk: 98h) = 7-day weekly reset countdown, (5h: 4h) = 5-hour rolling reset",
            ],
        };
        for n in notes {
            println!("\x1b[90m{}\x1b[0m", n);
        }
        println!();

        let mut items = Vec::new();
        for (i, acc) in snapshot.accounts.iter().enumerate() {
            let label = match &acc.custom_label {
                Some(l) if !l.trim().is_empty() => format!(" ({})", l.trim()),
                _ => String::new(),
            };
            let current = if acc.is_current { " *" } else { "" };
            items.push(format!("{}. {}{}{}", i + 1, acc.email, label, current));
        }
        let back_label = match lang {
            Lang::Zh => "0. 返回主菜单",
            Lang::En => "0. Back to main menu",
        };
        items.push(back_label.into());

        let title = match lang {
            Lang::Zh => "选择要查看独立模型配额的账号:",
            Lang::En => "Select account to view detailed model breakdown:",
        };
        let str_items: Vec<&str> = items.iter().map(String::as_str).collect();
        let sel = select_menu_interactive(title, &str_items, 0, lang);

        match sel {
            Some(idx) if idx < snapshot.accounts.len() => {
                show_single_account_quota(&snapshot.accounts[idx], lang);
            }
            _ => break,
        }
    }
}

fn show_single_account_quota(acc: &AccountView, lang: Lang) {
    print!("\x1b[2J\x1b[H");
    let title = match lang {
        Lang::Zh => format!("账号配额详情 · {}", acc.email),
        Lang::En => format!("Quota Details · {}", acc.email),
    };
    println!("\x1b[1m{}\x1b[0m\n", title);

    let status = if acc.disabled {
        match lang {
            Lang::Zh => "\x1b[31m已禁用\x1b[0m",
            Lang::En => "\x1b[31mDisabled\x1b[0m",
        }
    } else if acc.validation_blocked {
        match lang {
            Lang::Zh => "\x1b[33m需安全验证\x1b[0m",
            Lang::En => "\x1b[33mVerification Required\x1b[0m",
        }
    } else if acc.is_current {
        match lang {
            Lang::Zh => "\x1b[32m当前生效\x1b[0m",
            Lang::En => "\x1b[32mActive\x1b[0m",
        }
    } else {
        match lang {
            Lang::Zh => "正常",
            Lang::En => "Normal",
        }
    };

    let label_str = acc.custom_label.as_deref().unwrap_or("-");
    let tier_str = acc
        .quota
        .as_ref()
        .and_then(|q| q.subscription_tier.as_deref())
        .unwrap_or("-");

    let meta_headers = match lang {
        Lang::Zh => vec!["属性", "内容值"],
        Lang::En => vec!["Property", "Value"],
    };
    let mut meta_table = Table::new(meta_headers);
    let meta_rows = match lang {
        Lang::Zh => vec![
            vec!["邮箱地址".into(), acc.email.clone()],
            vec!["账号标识".into(), acc.id.clone()],
            vec!["备注标签".into(), label_str.into()],
            vec!["当前状态".into(), status.into()],
            vec!["订阅级别".into(), tier_str.into()],
        ],
        Lang::En => vec![
            vec!["Email".into(), acc.email.clone()],
            vec!["Account ID".into(), acc.id.clone()],
            vec!["Label".into(), label_str.into()],
            vec!["Status".into(), status.into()],
            vec!["Plan Tier".into(), tier_str.into()],
        ],
    };
    for r in meta_rows {
        meta_table.add_row(r);
    }
    print!("{}", meta_table.render());

    if let Some(q) = &acc.quota {
        if q.is_forbidden {
            let warn = match lang {
                Lang::Zh => "\n\x1b[31m[警告] 账号配额访问受限 (Forbidden)，可能需重新授权登录。\x1b[0m",
                Lang::En => "\n\x1b[31m[Warning] Quota access is forbidden; please re-authorize account.\x1b[0m",
            };
            println!("{}", warn);
        }

        let q_headers = match lang {
            Lang::Zh => vec!["配额窗口 / 模型", "余量", "进度", "重置倒计时"],
            Lang::En => vec!["Quota Window / Model", "Remaining", "Progress", "Resets In"],
        };
        let mut q_table = Table::new(q_headers);
        q_table.set_align_right(1, true);

        if let Some(groups) = &q.quota_groups {
            for g in groups {
                for b in &g.buckets {
                    let pct = (b.remaining_fraction * 100.0).round() as i32;
                    let bar = progress_bar(pct, 10);
                    let cd = format_countdown(&b.reset_time, lang);

                    let window_desc = if b.bucket_id.contains("week") || b.window.contains("week") {
                        match lang {
                            Lang::Zh => "周配额 (7天重置)",
                            Lang::En => "Weekly (7-Day)",
                        }
                    } else if b.bucket_id.contains("5h") || b.window.contains("5h") {
                        match lang {
                            Lang::Zh => "5小时滚动配额",
                            Lang::En => "5-Hour Rolling",
                        }
                    } else {
                        &b.bucket_id
                    };

                    let group_title = if g.display_name.contains("Gemini") {
                        "Gemini"
                    } else if g.display_name.contains("Claude") || g.display_name.contains("GPT") {
                        "Claude/GPT"
                    } else {
                        &g.display_name
                    };

                    let name = format!("{} ({})", group_title, window_desc);
                    q_table.add_row(vec![
                        name,
                        format!("{}%", pct),
                        bar,
                        if cd.is_empty() { "-".into() } else { cd },
                    ]);
                }
            }
        } else if !q.models.is_empty() {
            for m in &q.models {
                let bar = progress_bar(m.percentage, 10);
                let cd = format_countdown(&m.reset_time, lang);
                q_table.add_row(vec![
                    m.name.clone(),
                    format!("{}%", m.percentage),
                    bar,
                    if cd.is_empty() { "-".into() } else { cd },
                ]);
            }
        }

        println!("\n{}", q_table.render());
    } else {
        let msg = match lang {
            Lang::Zh => "\n\x1b[33m暂无本地缓存额度，请使用主菜单 [4] 刷新配额。\x1b[0m\n",
            Lang::En => "\n\x1b[33mNo cached quota data. Use [4] to refresh live quotas.\x1b[0m\n",
        };
        println!("{}", msg);
    }

    wait_for_key(lang);
}

fn show_token_statistics(lang: Lang) {
    print!("\x1b[2J\x1b[H");
    let title = match lang {
        Lang::Zh => "本地 Token 用量与预期费用统计",
        Lang::En => "Local Token Usage & Estimated Cost",
    };
    println!("\x1b[1m{}\x1b[0m\n", title);

    let wait_msg = match lang {
        Lang::Zh => "正在扫描本地 Antigravity 对话数据库...\n",
        Lang::En => "Scanning local Antigravity conversation databases...\n",
    };
    println!("{}", wait_msg);

    match crate::modules::native_token_stats::get_local_token_usage() {
        Ok(summary) => {
            println!("{}", format_token_stats_human(&summary, lang));
        }
        Err(e) => {
            let err_msg = match lang {
                Lang::Zh => format!("\x1b[31m读取本地 Token 统计失败: {}\x1b[0m", e),
                Lang::En => format!("\x1b[31mFailed to read token statistics: {}\x1b[0m", e),
            };
            println!("{}", err_msg);
        }
    }
    wait_for_key(lang);
}

fn show_refresh_quotas(snapshot: &Snapshot, lang: Lang) {
    if snapshot.accounts.is_empty() {
        let msg = match lang {
            Lang::Zh => "\n\x1b[33m暂无已保存账号。\x1b[0m",
            Lang::En => "\n\x1b[33mNo saved accounts.\x1b[0m",
        };
        println!("{}", msg);
        wait_for_key(lang);
        return;
    }

    let (title, items) = match lang {
        Lang::Zh => (
            "刷新配额选项:",
            vec![
                "1. 刷新当前生效账号配额",
                "2. 批量刷新全部账号 (并发执行)",
                "3. 选择指定账号刷新",
                "0. 返回主菜单",
            ],
        ),
        Lang::En => (
            "Refresh Options:",
            vec![
                "1. Refresh active account quota",
                "2. Batch refresh all accounts (concurrent)",
                "3. Select specific account to refresh",
                "0. Back to main menu",
            ],
        ),
    };

    let choice = select_menu_interactive(title, &items, 0, lang);

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            println!("\x1b[31mRuntime error: {}\x1b[0m", e);
            wait_for_key(lang);
            return;
        }
    };

    match choice {
        Some(0) => {
            let curr = match snapshot.current() {
                Ok(c) => c,
                Err(_) => {
                    let msg = match lang {
                        Lang::Zh => "\x1b[33m当前尚未设置生效账号，请使用 [3] 指定账号刷新。\x1b[0m",
                        Lang::En => "\x1b[33mNo active account set. Use [3] to select an account.\x1b[0m",
                    };
                    println!("{}", msg);
                    wait_for_key(lang);
                    return;
                }
            };
            let wait_msg = match lang {
                Lang::Zh => format!("\n\x1b[2m正在请求 Google API 刷新账号 ({}) 配额...\x1b[0m", curr.email),
                Lang::En => format!("\n\x1b[2mRefreshing quota for ({}) via Google API...\x1b[0m", curr.email),
            };
            println!("{}", wait_msg);
            match runtime.block_on(async {
                let mut account = crate::modules::account::load_account(&curr.id)?;
                let quota = crate::modules::account::fetch_quota_with_retry(&mut account)
                    .await
                    .map_err(|e| e.to_string())?;
                crate::modules::account::update_account_quota(&curr.id, quota.clone())?;
                Ok::<crate::models::QuotaData, String>(quota)
            }) {
                Ok(quota) => {
                    let succ_msg = match lang {
                        Lang::Zh => "\x1b[1;32m✓ 配额刷新成功！最新数据:\x1b[0m",
                        Lang::En => "\x1b[1;32m✓ Quota refreshed successfully! Latest:\x1b[0m",
                    };
                    println!("{}", succ_msg);

                    let q_headers = match lang {
                        Lang::Zh => vec!["模型 / 分组", "剩余比例", "重置倒计时"],
                        Lang::En => vec!["Model / Group", "Remaining", "Resets In"],
                    };
                    let mut q_table = Table::new(q_headers);
                    q_table.set_align_right(1, true);

                    if let Some(groups) = &quota.quota_groups {
                        for g in groups {
                            if let Some(b) = g.buckets.first() {
                                let pct = (b.remaining_fraction * 100.0).round() as i32;
                                let cd = format_countdown(&b.reset_time, lang);
                                q_table.add_row(vec![
                                    g.display_name.clone(),
                                    format!("{}%", pct),
                                    if cd.is_empty() { "-".into() } else { cd },
                                ]);
                            }
                        }
                    } else {
                        for m in &quota.models {
                            let cd = format_countdown(&m.reset_time, lang);
                            q_table.add_row(vec![
                                m.name.clone(),
                                format!("{}%", m.percentage),
                                if cd.is_empty() { "-".into() } else { cd },
                            ]);
                        }
                    }
                    print!("{}", q_table.render());
                }
                Err(e) => {
                    let fail_msg = match lang {
                        Lang::Zh => format!("\x1b[31m刷新失败: {}\x1b[0m", e),
                        Lang::En => format!("\x1b[31mRefresh failed: {}\x1b[0m", e),
                    };
                    println!("{}", fail_msg);
                }
            }
            wait_for_key(lang);
        }
        Some(1) => {
            let wait_msg = match lang {
                Lang::Zh => "\n\x1b[2m正在并发批量刷新所有账号配额...\x1b[0m",
                Lang::En => "\n\x1b[2mBatch refreshing all accounts concurrently...\x1b[0m",
            };
            println!("{}", wait_msg);
            match runtime.block_on(crate::modules::account::refresh_all_quotas_logic()) {
                Ok(stats) => {
                    let result_msg = match lang {
                        Lang::Zh => format!(
                            "\x1b[1;32m✓ 批量刷新完成: 成功 {} 个, 失败 {} 个 (总计: {})\x1b[0m",
                            stats.success, stats.failed, stats.total
                        ),
                        Lang::En => format!(
                            "\x1b[1;32m✓ Batch refresh complete: {} succeeded, {} failed (total: {})\x1b[0m",
                            stats.success, stats.failed, stats.total
                        ),
                    };
                    println!("{}", result_msg);
                    for detail in stats.details {
                        println!("  {}", detail);
                    }
                }
                Err(e) => {
                    let fail_msg = match lang {
                        Lang::Zh => format!("\x1b[31m批量刷新失败: {}\x1b[0m", e),
                        Lang::En => format!("\x1b[31mBatch refresh failed: {}\x1b[0m", e),
                    };
                    println!("{}", fail_msg);
                }
            }
            wait_for_key(lang);
        }
        Some(2) => {
            let selected = match select_account_interactive(&snapshot.accounts, lang) {
                Some(acc) => acc,
                None => return,
            };
            let wait_msg = match lang {
                Lang::Zh => format!("\n\x1b[2m正在刷新账号 ({}) 配额...\x1b[0m", selected.email),
                Lang::En => format!("\n\x1b[2mRefreshing quota for ({}) ...\x1b[0m", selected.email),
            };
            println!("{}", wait_msg);
            match runtime.block_on(async {
                let mut account = crate::modules::account::load_account(&selected.id)?;
                let quota = crate::modules::account::fetch_quota_with_retry(&mut account)
                    .await
                    .map_err(|e| e.to_string())?;
                crate::modules::account::update_account_quota(&selected.id, quota.clone())?;
                Ok::<crate::models::QuotaData, String>(quota)
            }) {
                Ok(quota) => {
                    let succ_msg = match lang {
                        Lang::Zh => "\x1b[1;32m✓ 配额刷新成功！\x1b[0m",
                        Lang::En => "\x1b[1;32m✓ Quota refreshed successfully!\x1b[0m",
                    };
                    println!("{}", succ_msg);

                    let q_headers = match lang {
                        Lang::Zh => vec!["模型 / 分组", "剩余比例", "重置倒计时"],
                        Lang::En => vec!["Model / Group", "Remaining", "Resets In"],
                    };
                    let mut q_table = Table::new(q_headers);
                    q_table.set_align_right(1, true);

                    if let Some(groups) = &quota.quota_groups {
                        for g in groups {
                            if let Some(b) = g.buckets.first() {
                                let pct = (b.remaining_fraction * 100.0).round() as i32;
                                let cd = format_countdown(&b.reset_time, lang);
                                q_table.add_row(vec![
                                    g.display_name.clone(),
                                    format!("{}%", pct),
                                    if cd.is_empty() { "-".into() } else { cd },
                                ]);
                            }
                        }
                    } else {
                        for m in &quota.models {
                            let cd = format_countdown(&m.reset_time, lang);
                            q_table.add_row(vec![
                                m.name.clone(),
                                format!("{}%", m.percentage),
                                if cd.is_empty() { "-".into() } else { cd },
                            ]);
                        }
                    }
                    print!("{}", q_table.render());
                }
                Err(e) => {
                    let fail_msg = match lang {
                        Lang::Zh => format!("\x1b[31m刷新失败: {}\x1b[0m", e),
                        Lang::En => format!("\x1b[31mRefresh failed: {}\x1b[0m", e),
                    };
                    println!("{}", fail_msg);
                }
            }
            wait_for_key(lang);
        }
        _ => {}
    }
}

fn show_add_account(lang: Lang) {
    let (title, items) = match lang {
        Lang::Zh => (
            "添加 Google 账号:",
            vec![
                "1. 浏览器一键授权 (Google OAuth 自动登录)",
                "2. 手动输入 Refresh Token",
                "0. 返回主菜单",
            ],
        ),
        Lang::En => (
            "Add Google Account:",
            vec![
                "1. Browser authorization (Google OAuth auto-login)",
                "2. Manually enter Refresh Token",
                "0. Back to main menu",
            ],
        ),
    };

    let choice = select_menu_interactive(title, &items, 0, lang);

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            println!("\x1b[31mRuntime error: {}\x1b[0m", e);
            wait_for_key(lang);
            return;
        }
    };

    match choice {
        Some(0) => {
            let wait_msg = match lang {
                Lang::Zh => "\n\x1b[2m正在准备 Google OAuth 登录服务...\x1b[0m",
                Lang::En => "\n\x1b[2mPreparing Google OAuth login service...\x1b[0m",
            };
            println!("{}", wait_msg);

            let auth_url = match runtime.block_on(crate::modules::oauth_server::prepare_oauth_url(None, None)) {
                Ok(url) => url,
                Err(e) => {
                    let err_msg = match lang {
                        Lang::Zh => format!("\x1b[31m准备授权服务失败: {}\x1b[0m", e),
                        Lang::En => format!("\x1b[31mFailed to prepare authorization: {}\x1b[0m", e),
                    };
                    println!("{}", err_msg);
                    wait_for_key(lang);
                    return;
                }
            };

            open_browser(&auth_url);
            let open_msg = match lang {
                Lang::Zh => format!(
                    "\x1b[1;32m✓ 已启动本地回调服务并尝试打开系统浏览器。\x1b[0m\n若浏览器未自动打开，请手动复制并在浏览器中访问以下授权链接:\n\x1b[4;34m{}\x1b[0m\n\n\x1b[2m等待浏览器授权完成... (可按 Ctrl+C 取消)\x1b[0m",
                    auth_url
                ),
                Lang::En => format!(
                    "\x1b[1;32m✓ Local callback service started. Opening browser.\x1b[0m\nIf browser does not open automatically, copy and visit this URL:\n\x1b[4;34m{}\x1b[0m\n\n\x1b[2mWaiting for browser authorization... (Press Ctrl+C to cancel)\x1b[0m",
                    auth_url
                ),
            };
            println!("{}", open_msg);

            let token_res = match runtime.block_on(crate::modules::oauth_server::complete_oauth_flow(None)) {
                Ok(t) => t,
                Err(e) => {
                    let fail_msg = match lang {
                        Lang::Zh => format!("\x1b[31m授权流程失败: {}\x1b[0m", e),
                        Lang::En => format!("\x1b[31mAuthorization failed: {}\x1b[0m", e),
                    };
                    println!("{}", fail_msg);
                    wait_for_key(lang);
                    return;
                }
            };

            let refresh_token = match token_res.refresh_token {
                Some(rt) => rt,
                None => {
                    let no_token_msg = match lang {
                        Lang::Zh => "\x1b[31m未能获取到 Refresh Token，请撤销旧授权后重试。\x1b[0m",
                        Lang::En => "\x1b[31mFailed to obtain Refresh Token. Revoke prior access and retry.\x1b[0m",
                    };
                    println!("{}", no_token_msg);
                    wait_for_key(lang);
                    return;
                }
            };

            match runtime.block_on(async {
                let user_info = crate::modules::oauth::get_user_info(&token_res.access_token).await?;
                let project_id = crate::modules::project_resolver::fetch_project_id(&token_res.access_token)
                    .await
                    .ok();
                let token_data = crate::models::TokenData::new(
                    token_res.access_token.clone(),
                    refresh_token,
                    token_res.expires_in,
                    Some(user_info.email.clone()),
                    project_id,
                    None,
                    false,
                    token_res.id_token.clone(),
                )
                .with_oauth_client_key(token_res.oauth_client_key.clone());

                let mut account = crate::modules::upsert_account(
                    user_info.email.clone(),
                    user_info.get_display_name(),
                    token_data,
                )?;

                let _ = crate::modules::account::fetch_quota_with_retry(&mut account).await;
                Ok::<String, String>(account.email)
            }) {
                Ok(email) => {
                    let succ_msg = match lang {
                        Lang::Zh => format!("\x1b[1;32m✓ 账号添加成功: {}\x1b[0m", email),
                        Lang::En => format!("\x1b[1;32m✓ Account added successfully: {}\x1b[0m", email),
                    };
                    println!("{}", succ_msg);
                }
                Err(e) => {
                    let fail_msg = match lang {
                        Lang::Zh => format!("\x1b[31m添加账号失败: {}\x1b[0m", e),
                        Lang::En => format!("\x1b[31mFailed to add account: {}\x1b[0m", e),
                    };
                    println!("{}", fail_msg);
                }
            }
            wait_for_key(lang);
        }
        Some(1) => {
            let prompt_text = match lang {
                Lang::Zh => "\n请输入 Google Refresh Token: ",
                Lang::En => "\nEnter Google Refresh Token: ",
            };
            let refresh_token = prompt_line(prompt_text);
            if refresh_token.is_empty() {
                let cancel_msg = match lang {
                    Lang::Zh => "\x1b[2m已取消输入。\x1b[0m",
                    Lang::En => "\x1b[2mInput cancelled.\x1b[0m",
                };
                println!("{}", cancel_msg);
                wait_for_key(lang);
                return;
            }

            let wait_msg = match lang {
                Lang::Zh => "\x1b[2m正在验证 Token 并拉取账号详情...\x1b[0m",
                Lang::En => "\x1b[2mVerifying Token and fetching profile...\x1b[0m",
            };
            println!("{}", wait_msg);

            match runtime.block_on(async {
                let token_res = crate::modules::oauth::refresh_access_token(&refresh_token, None).await?;
                let user_info = crate::modules::oauth::get_user_info(&token_res.access_token).await?;
                let project_id = crate::modules::project_resolver::fetch_project_id(&token_res.access_token)
                    .await
                    .ok();
                let token_data = crate::models::TokenData::new(
                    token_res.access_token.clone(),
                    refresh_token,
                    token_res.expires_in,
                    Some(user_info.email.clone()),
                    project_id,
                    None,
                    false,
                    token_res.id_token.clone(),
                )
                .with_oauth_client_key(token_res.oauth_client_key.clone());

                let mut account = crate::modules::upsert_account(
                    user_info.email.clone(),
                    user_info.get_display_name(),
                    token_data,
                )?;

                let _ = crate::modules::account::fetch_quota_with_retry(&mut account).await;
                Ok::<String, String>(account.email)
            }) {
                Ok(email) => {
                    let succ_msg = match lang {
                        Lang::Zh => format!("\x1b[1;32m✓ 账号导入成功: {}\x1b[0m", email),
                        Lang::En => format!("\x1b[1;32m✓ Account imported successfully: {}\x1b[0m", email),
                    };
                    println!("{}", succ_msg);
                }
                Err(e) => {
                    let fail_msg = match lang {
                        Lang::Zh => format!("\x1b[31m导入失败: {}\x1b[0m", e),
                        Lang::En => format!("\x1b[31mImport failed: {}\x1b[0m", e),
                    };
                    println!("{}", fail_msg);
                }
            }
            wait_for_key(lang);
        }
        _ => {}
    }
}

fn show_manage_accounts(snapshot: &Snapshot, lang: Lang) {
    if snapshot.accounts.is_empty() {
        let msg = match lang {
            Lang::Zh => "\n\x1b[33m暂无已保存账号。\x1b[0m",
            Lang::En => "\n\x1b[33mNo saved accounts.\x1b[0m",
        };
        println!("{}", msg);
        wait_for_key(lang);
        return;
    }

    print!("\x1b[2J\x1b[H");
    let header = match lang {
        Lang::Zh => "账号管理与设置",
        Lang::En => "Account Management",
    };
    println!("\x1b[1m{}\x1b[0m\n", header);

    let selected = match select_account_interactive(&snapshot.accounts, lang) {
        Some(acc) => acc,
        None => return,
    };

    print!("\x1b[2J\x1b[H");

    let (title, sub_items) = match lang {
        Lang::Zh => (
            format!("管理账号: {} (ID: {})", selected.email, selected.id),
            vec![
                "1. 修改备注标签 (最多15字)",
                "2. 切换启用/禁用状态",
                "3. 删除账号 (永久移除)",
                "0. 返回主菜单",
            ],
        ),
        Lang::En => (
            format!("Manage Account: {} (ID: {})", selected.email, selected.id),
            vec![
                "1. Edit Custom Label (max 15 chars)",
                "2. Toggle Enable / Disable",
                "3. Delete Account (permanent)",
                "0. Back to main menu",
            ],
        ),
    };

    let sub_choice = select_menu_interactive(&title, &sub_items, 0, lang);

    match sub_choice {
        Some(0) => {
            let prompt = match lang {
                Lang::Zh => format!(
                    "\n当前备注: {}\n请输入新的备注标签 (直接回车可清除当前备注): ",
                    selected.custom_label.as_deref().unwrap_or("无")
                ),
                Lang::En => format!(
                    "\nCurrent Label: {}\nEnter new label (Press Enter to clear): ",
                    selected.custom_label.as_deref().unwrap_or("None")
                ),
            };
            let new_label = prompt_line(&prompt);
            if new_label.chars().count() > 15 {
                let err_msg = match lang {
                    Lang::Zh => "\x1b[31m标签长度不能超过 15 个字符\x1b[0m",
                    Lang::En => "\x1b[31mLabel cannot exceed 15 characters\x1b[0m",
                };
                println!("{}", err_msg);
            } else {
                match crate::modules::account::load_account(&selected.id) {
                    Ok(mut acc) => {
                        acc.custom_label = if new_label.is_empty() {
                            None
                        } else {
                            Some(new_label.clone())
                        };
                        if let Err(e) = crate::modules::account::save_account(&acc) {
                            println!("\x1b[31mSave error: {}\x1b[0m", e);
                        } else if new_label.is_empty() {
                            let clr_msg = match lang {
                                Lang::Zh => "\x1b[1;32m✓ 备注标签已成功清除\x1b[0m",
                                Lang::En => "\x1b[1;32m✓ Label cleared successfully\x1b[0m",
                            };
                            println!("{}", clr_msg);
                        } else {
                            let upd_msg = match lang {
                                Lang::Zh => format!("\x1b[1;32m✓ 备注标签已更新为: {}\x1b[0m", new_label),
                                Lang::En => format!("\x1b[1;32m✓ Label updated to: {}\x1b[0m", new_label),
                            };
                            println!("{}", upd_msg);
                        }
                    }
                    Err(e) => println!("\x1b[31mLoad error: {}\x1b[0m", e),
                }
            }
            wait_for_key(lang);
        }
        Some(1) => {
            match crate::modules::account::load_account(&selected.id) {
                Ok(mut acc) => {
                    acc.disabled = !acc.disabled;
                    if acc.disabled {
                        acc.disabled_at = Some(chrono::Utc::now().timestamp());
                        acc.disabled_reason = Some("Manually disabled via CLI".to_string());
                    } else {
                        acc.disabled_at = None;
                        acc.disabled_reason = None;
                    }
                    if let Err(e) = crate::modules::account::save_account(&acc) {
                        println!("\x1b[31mSave error: {}\x1b[0m", e);
                    } else {
                        let status_text = if acc.disabled {
                            match lang {
                                Lang::Zh => "\x1b[33m已禁用\x1b[0m",
                                Lang::En => "\x1b[33mDisabled\x1b[0m",
                            }
                        } else {
                            match lang {
                                Lang::Zh => "\x1b[32m正常启用\x1b[0m",
                                Lang::En => "\x1b[32mEnabled\x1b[0m",
                            }
                        };
                        let toggle_msg = match lang {
                            Lang::Zh => format!("\x1b[1;32m✓ 账号状态已切换为: {}\x1b[0m", status_text),
                            Lang::En => format!("\x1b[1;32m✓ Account status changed to: {}\x1b[0m", status_text),
                        };
                        println!("{}", toggle_msg);
                    }
                }
                Err(e) => println!("\x1b[31mLoad error: {}\x1b[0m", e),
            }
            wait_for_key(lang);
        }
        Some(2) => {
            let confirm_prompt = match lang {
                Lang::Zh => format!(
                    "\n确定要永久删除账号 {} 吗？此操作无法撤销！(y/N): ",
                    selected.email
                ),
                Lang::En => format!(
                    "\nPermanently delete account {}? Cannot be undone! (y/N): ",
                    selected.email
                ),
            };
            let confirm = prompt_line(&confirm_prompt);
            if confirm.eq_ignore_ascii_case("y") || confirm.eq_ignore_ascii_case("yes") {
                match crate::modules::account::delete_account(&selected.id) {
                    Ok(_) => {
                        let del_msg = match lang {
                            Lang::Zh => "\x1b[1;32m✓ 账号已成功删除\x1b[0m",
                            Lang::En => "\x1b[1;32m✓ Account deleted successfully\x1b[0m",
                        };
                        println!("{}", del_msg);
                    }
                    Err(e) => println!("\x1b[31mDelete error: {}\x1b[0m", e),
                }
            } else {
                let cancel_msg = match lang {
                    Lang::Zh => "\x1b[2m已取消删除操作。\x1b[0m",
                    Lang::En => "\x1b[2mDeletion cancelled.\x1b[0m",
                };
                println!("{}", cancel_msg);
            }
            wait_for_key(lang);
        }
        _ => {}
    }
}

fn show_system_status(snapshot: &Snapshot, root: &Path, lang: Lang) {
    print!("\x1b[2J\x1b[H");
    let title = match lang {
        Lang::Zh => "关联应用与系统状态",
        Lang::En => "Linked Applications & System Status",
    };
    println!("\x1b[1m{}\x1b[0m\n", title);

    let home = std::env::var("HOME").unwrap_or_default();
    let app_path = crate::modules::process::get_antigravity_executable_path(None);
    let app_running = crate::modules::process::is_antigravity_running(None);
    let ide_running = crate::modules::process::is_antigravity_running(Some("ide"));

    let running_text = match lang {
        Lang::Zh => "\x1b[32m运行中\x1b[0m",
        Lang::En => "\x1b[32mRunning\x1b[0m",
    };
    let stopped_text = match lang {
        Lang::Zh => "\x1b[90m未运行\x1b[0m",
        Lang::En => "\x1b[90mStopped\x1b[0m",
    };

    let app_status = if app_running { running_text } else { stopped_text };
    let ide_status = if ide_running { running_text } else { stopped_text };

    let app_path_str = if let Some(p) = app_path {
        if !home.is_empty() && p.starts_with(&home) {
            let rel = p.strip_prefix(&home).unwrap_or(&p);
            format!("~/{}", rel.display())
        } else {
            p.display().to_string()
        }
    } else {
        match lang {
            Lang::Zh => "未自动识别 (可在设置中手动指定)".into(),
            Lang::En => "Not detected (can specify in Settings)".into(),
        }
    };

    let ide_details = match lang {
        Lang::Zh => "支持独立通道关联",
        Lang::En => "Independent IDE channel supported",
    };

    let active_info = if let Ok(curr) = snapshot.current() {
        format!(
            "{} [{}]",
            curr.email,
            snapshot.current_target.as_deref().unwrap_or("app")
        )
    } else {
        match lang {
            Lang::Zh => "未设置".into(),
            Lang::En => "None".into(),
        }
    };

    let display_root = if !home.is_empty() && root.starts_with(&home) {
        let rel = root.strip_prefix(&home).unwrap_or(root);
        format!("~/{}", rel.display())
    } else {
        root.display().to_string()
    };

    let storage_status = match lang {
        Lang::Zh => format!("{} (共 {} 个账号)", display_root, snapshot.accounts.len()),
        Lang::En => format!("{} ({} accounts)", display_root, snapshot.accounts.len()),
    };

    let headers = match lang {
        Lang::Zh => vec!["组件名称", "状态", "路径 / 详细信息"],
        Lang::En => vec!["Component", "Status", "Path / Details"],
    };
    let mut table = Table::new(headers);

    let rows = match lang {
        Lang::Zh => vec![
            vec!["AntiGravity 桌面应用".into(), app_status.into(), app_path_str],
            vec!["AntiGravity 独立环境".into(), ide_status.into(), ide_details.into()],
            vec!["本地数据存储".into(), "正常".into(), storage_status],
            vec!["当前生效账号".into(), "生效".into(), active_info],
            vec![
                "命令行工具".into(),
                format!("v{}", env!("CARGO_PKG_VERSION")),
                "agy-switch".into(),
            ],
        ],
        Lang::En => vec![
            vec!["AntiGravity Desktop App".into(), app_status.into(), app_path_str],
            vec!["AntiGravity IDE Channel".into(), ide_status.into(), ide_details.into()],
            vec!["Local Data Storage".into(), "Normal".into(), storage_status],
            vec!["Active Account".into(), "Active".into(), active_info],
            vec![
                "CLI Binary".into(),
                format!("v{}", env!("CARGO_PKG_VERSION")),
                "agy-switch".into(),
            ],
        ],
    };

    for r in rows {
        table.add_row(r);
    }
    print!("{}", table.render());

    wait_for_key(lang);
}
