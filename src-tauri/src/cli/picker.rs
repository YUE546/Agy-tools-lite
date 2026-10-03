use super::output::{AccountView, QuotaView};
use std::io::{self, Read, Write};

fn quota_brief(quota: Option<&QuotaView>) -> String {
    if let Some(q) = quota {
        if q.is_forbidden {
            return " (额度受限)".into();
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
            // Non-blocking timeout for reading multi-byte escape sequences
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
    None,
}

#[cfg(unix)]
fn read_key_action() -> KeyAction {
    let mut byte = [0u8; 1];
    let mut stdin = io::stdin();

    // Read first byte (wait up to 100ms per VTIME)
    loop {
        match stdin.read(&mut byte) {
            Ok(1) => break,
            Ok(0) => continue, // keep waiting
            _ => return KeyAction::Cancel,
        }
    }

    match byte[0] {
        b'\r' | b'\n' => KeyAction::Enter,
        b'\x03' | b'q' | b'Q' => KeyAction::Cancel,
        b'k' | b'K' => KeyAction::Up,
        b'j' | b'J' => KeyAction::Down,
        b'1'..=b'9' => KeyAction::SelectIndex((byte[0] - b'1') as usize),
        b'\x1b' => {
            // Escape sequence or standalone Esc
            let mut seq = [0u8; 2];
            match stdin.read(&mut seq[0..1]) {
                Ok(1) if seq[0] == b'[' => {
                    match stdin.read(&mut seq[1..2]) {
                        Ok(1) => match seq[1] {
                            b'A' => KeyAction::Up,
                            b'B' => KeyAction::Down,
                            _ => KeyAction::None,
                        },
                        _ => KeyAction::Cancel,
                    }
                }
                _ => KeyAction::Cancel,
            }
        }
        _ => KeyAction::None,
    }
}

pub fn select_account_interactive<'a>(accounts: &'a [AccountView]) -> Option<&'a AccountView> {
    if accounts.is_empty() {
        return None;
    }

    #[cfg(unix)]
    {
        if let Some(_raw) = RawTerminal::enter() {
            let mut selected = accounts.iter().position(|a| a.is_current).unwrap_or(0);
            let mut stdout = io::stdout();

            // Hide cursor
            print!("\x1b[?25l");
            let _ = stdout.flush();

            let render = |sel: usize, initial: bool| {
                let mut out = io::stdout();
                if !initial {
                    // Move cursor up to overwrite previous render
                    print!("\x1b[{}A", accounts.len() + 1);
                }
                // Header
                println!("\x1b[2K\r\x1b[1;36m? 请选择要切换生效的账号\x1b[0m \x1b[2m(↑/↓ 移动，回车确认，q/Esc 取消):\x1b[0m");

                for (i, acc) in accounts.iter().enumerate() {
                    let is_sel = i == sel;
                    let quota_text = quota_brief(acc.quota.as_ref());
                    let mut flags = Vec::new();
                    if acc.is_current {
                        flags.push("当前生效");
                    }
                    if acc.disabled {
                        flags.push("已禁用");
                    }
                    let flag_str = if flags.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", flags.join(", "))
                    };

                    if is_sel {
                        println!(
                            "\x1b[2K\r \x1b[1;32m❯\x1b[0m \x1b[1;37m{:<32}\x1b[0m\x1b[36m{}\x1b[0m\x1b[33m{}\x1b[0m",
                            acc.email, quota_text, flag_str
                        );
                    } else {
                        println!(
                            "\x1b[2K\r   \x1b[2m{:<32}{}{}\x1b[0m",
                            acc.email, quota_text, flag_str
                        );
                    }
                }
                let _ = out.flush();
            };

            render(selected, true);

            loop {
                match read_key_action() {
                    KeyAction::Up => {
                        if selected > 0 {
                            selected -= 1;
                        } else {
                            selected = accounts.len() - 1;
                        }
                        render(selected, false);
                    }
                    KeyAction::Down => {
                        if selected + 1 < accounts.len() {
                            selected += 1;
                        } else {
                            selected = 0;
                        }
                        render(selected, false);
                    }
                    KeyAction::SelectIndex(idx) => {
                        if idx < accounts.len() {
                            selected = idx;
                            render(selected, false);
                        }
                    }
                    KeyAction::Enter => {
                        // Clear interactive block and print confirmation
                        print!("\x1b[{}A", accounts.len() + 1);
                        for _ in 0..=accounts.len() {
                            println!("\x1b[2K\r");
                        }
                        print!("\x1b[{}A", accounts.len() + 1);
                        print!("\x1b[?25h");
                        let _ = stdout.flush();
                        return Some(&accounts[selected]);
                    }
                    KeyAction::Cancel => {
                        // Clear interactive block
                        print!("\x1b[{}A", accounts.len() + 1);
                        for _ in 0..=accounts.len() {
                            println!("\x1b[2K\r");
                        }
                        print!("\x1b[{}A", accounts.len() + 1);
                        print!("\x1b[?25h");
                        println!("\x1b[2m已取消选择。\x1b[0m");
                        let _ = stdout.flush();
                        return None;
                    }
                    KeyAction::None => {}
                }
            }
        }
    }

    // Fallback for non-TTY or non-Unix platforms
    println!("请选择要切换的账号 (输入序号 1-{}):", accounts.len());
    for (i, acc) in accounts.iter().enumerate() {
        let quota_text = quota_brief(acc.quota.as_ref());
        let current_marker = if acc.is_current { " [当前生效]" } else { "" };
        println!("  {}. {}{}{}", i + 1, acc.email, quota_text, current_marker);
    }
    print!("请输入序号 [1-{}]: ", accounts.len());
    let _ = io::stdout().flush();
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        if let Ok(num) = input.trim().parse::<usize>() {
            if num >= 1 && num <= accounts.len() {
                return Some(&accounts[num - 1]);
            }
        }
    }
    None
}
