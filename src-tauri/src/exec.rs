use crate::config::TypingMode;
use crate::engine::Job;
use crate::win::*;
use std::sync::OnceLock;
use std::time::Duration;

pub const MAGIC: usize = 0x4B46_4C57_0000_0001;

static PASTE_FN: OnceLock<Box<dyn Fn(&str) -> bool + Send + Sync>> = OnceLock::new();

pub fn set_paste_fn(f: Box<dyn Fn(&str) -> bool + Send + Sync>) {
    let _ = PASTE_FN.set(f);
}

#[derive(Clone, Copy)]
pub struct SendHwnd(pub HWND);
unsafe impl Send for SendHwnd {}

static LOG_TX: OnceLock<std::sync::mpsc::Sender<String>> = OnceLock::new();

fn log_sender() -> &'static std::sync::mpsc::Sender<String> {
    LOG_TX.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let _ = std::thread::Builder::new()
            .name("kf-log".to_string())
            .spawn(move || {
                use std::io::Write;
                let dir = crate::config::data_dir();
                let _ = std::fs::create_dir_all(&dir);
                let path = dir.join("keyflow.log");
                let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path).ok();
                for msg in rx {
                    if file.is_none() {
                        file = std::fs::OpenOptions::new().create(true).append(true).open(&path).ok();
                    }
                    if let Some(f) = file.as_mut() {
                        let _ = writeln!(f, "{}", msg);
                        let _ = f.flush();
                    }
                }
            });
        tx
    })
}

/// Never blocks the caller. The low-level keyboard hook has a strict time budget,
/// and a slow hook gets removed by Windows without any warning.
pub fn logline(msg: &str) {
    let _ = log_sender().send(msg.to_string());
}

/// Writes on the calling thread, for messages that must be on disk before the
/// process stops (panics, startup, clean exit).
pub fn logline_sync(msg: &str) {
    use std::io::Write;
    let dir = crate::config::data_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("keyflow.log"))
    {
        let _ = writeln!(f, "{}", msg);
    }
}

pub fn own_foreground() -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_null() {
            return false;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(fg, &mut pid);
        pid == GetCurrentProcessId()
    }
}

pub fn capture_foreground() -> Option<SendHwnd> {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_null() {
            return None;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(fg, &mut pid);
        if pid == GetCurrentProcessId() {
            return None;
        }
        Some(SendHwnd(fg))
    }
}

fn unicode_input(ch: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        u: INPUT_UNION {
            ki: KEYBDINPUT {
                wVk: 0,
                wScan: ch,
                dwFlags: KEYEVENTF_UNICODE | if up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: MAGIC,
            },
        },
    }
}

fn vk_input(vk: u16, up: bool, extended: bool) -> INPUT {
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) as u16 };
    let mut flags: u32 = 0;
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        u: INPUT_UNION {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: MAGIC,
            },
        },
    }
}

fn mouse_input(flags: u32) -> INPUT {
    INPUT {
        r#type: 0,
        u: INPUT_UNION {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: MAGIC,
            },
        },
    }
}

fn send(events: &[INPUT]) {
    if events.is_empty() {
        return;
    }
    unsafe {
        SendInput(
            events.len() as u32,
            events.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
    }
}

pub fn tap_vk(vk: u16, extended: bool) {
    let ev = [vk_input(vk, false, extended), vk_input(vk, true, extended)];
    send(&ev);
}

pub fn combo(mods: &[u16], vk: u16, extended: bool) {
    let mut ev: Vec<INPUT> = Vec::with_capacity(mods.len() * 2 + 2);
    for m in mods {
        ev.push(vk_input(*m, false, false));
    }
    ev.push(vk_input(vk, false, extended));
    ev.push(vk_input(vk, true, extended));
    for m in mods.iter().rev() {
        ev.push(vk_input(*m, true, false));
    }
    send(&ev);
}

pub fn backspaces(n: usize) {
    let mut ev: Vec<INPUT> = Vec::with_capacity(n * 2);
    for _ in 0..n {
        ev.push(vk_input(VK_BACK as u16, false, false));
        ev.push(vk_input(VK_BACK as u16, true, false));
    }
    send(&ev);
}

#[allow(dead_code)]
pub fn type_text_instant(text: &str) {
    let units: Vec<u16> = text.encode_utf16().collect();
    for chunk in units.chunks(24) {
        let mut ev: Vec<INPUT> = Vec::with_capacity(chunk.len() * 2);
        for u in chunk {
            ev.push(unicode_input(*u, false));
            ev.push(unicode_input(*u, true));
        }
        send(&ev);
    }
}

pub fn type_char(ch: u16) {
    let ev = [unicode_input(ch, false), unicode_input(ch, true)];
    send(&ev);
}

pub enum Item {
    Text(String),
    Key {
        vk: u16,
        mods: Vec<u16>,
        extended: bool,
    },
    Sleep(u64),
    Click {
        down: u32,
        up: u32,
    },
}

pub fn tokenize(text: &str) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    let mut buf = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\n' => {
                flush(&mut buf, &mut items);
                items.push(Item::Key {
                    vk: VK_RETURN as u16,
                    mods: vec![],
                    extended: false,
                });
            }
            '\t' => {
                flush(&mut buf, &mut items);
                items.push(Item::Key {
                    vk: VK_TAB as u16,
                    mods: vec![],
                    extended: false,
                });
            }
            '{' => {
                let mut token = String::new();
                let mut closed = false;
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if n == '}' {
                        closed = true;
                        break;
                    }
                    token.push(n);
                }
                if closed {
                    match parse_token(&token) {
                        Some(item) => {
                            flush(&mut buf, &mut items);
                            items.push(item);
                        }
                        None => {
                            buf.push('{');
                            buf.push_str(&token);
                            buf.push('}');
                        }
                    }
                } else {
                    buf.push('{');
                    buf.push_str(&token);
                }
            }
            other => buf.push(other),
        }
    }
    flush(&mut buf, &mut items);
    items
}

fn flush(buf: &mut String, items: &mut Vec<Item>) {
    if !buf.is_empty() {
        items.push(Item::Text(std::mem::take(buf)));
    }
}

fn parse_token(raw: &str) -> Option<Item> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    let up = t.to_uppercase();
    if let Some(rest) = up.strip_prefix("SLEEP") {
        let n = rest.trim().trim_end_matches("MS").trim();
        if let Ok(ms) = n.parse::<u64>() {
            return Some(Item::Sleep(ms));
        }
        return None;
    }
    if up == "LBUTTON" || up == "LCLICK" {
        return Some(Item::Click {
            down: MOUSEEVENTF_LEFTDOWN,
            up: MOUSEEVENTF_LEFTUP,
        });
    }
    if up == "RBUTTON" || up == "RCLICK" {
        return Some(Item::Click {
            down: MOUSEEVENTF_RIGHTDOWN,
            up: MOUSEEVENTF_RIGHTUP,
        });
    }
    if up == "MBUTTON" || up == "MCLICK" {
        return Some(Item::Click {
            down: MOUSEEVENTF_MIDDLEDOWN,
            up: MOUSEEVENTF_MIDDLEUP,
        });
    }
    let parts: Vec<&str> = up.split('+').map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }
    let mut mods: Vec<u16> = Vec::new();
    for m in &parts[..parts.len() - 1] {
        match *m {
            "CTRL" | "CONTROL" => mods.push(VK_CONTROL as u16),
            "SHIFT" => mods.push(VK_SHIFT as u16),
            "ALT" => mods.push(VK_MENU as u16),
            "WIN" | "WINDOWS" | "META" => mods.push(VK_LWIN as u16),
            _ => return None,
        }
    }
    let (vk, extended) = key_vk(parts[parts.len() - 1])?;
    Some(Item::Key { vk, mods, extended })
}

fn key_vk(name: &str) -> Option<(u16, bool)> {
    let simple = match name {
        "ENTER" | "RETURN" => Some((VK_RETURN, false)),
        "TAB" => Some((VK_TAB, false)),
        "SPACE" => Some((VK_SPACE, false)),
        "BACKSPACE" | "BS" => Some((VK_BACK, false)),
        "DELETE" | "DEL" => Some((VK_DELETE, true)),
        "ESC" | "ESCAPE" => Some((VK_ESCAPE, false)),
        "UP" => Some((VK_UP, true)),
        "DOWN" => Some((VK_DOWN, true)),
        "LEFT" => Some((VK_LEFT, true)),
        "RIGHT" => Some((VK_RIGHT, true)),
        "HOME" => Some((VK_HOME, true)),
        "END" => Some((VK_END, true)),
        "PGUP" | "PAGEUP" => Some((VK_PRIOR, true)),
        "PGDN" | "PAGEDOWN" => Some((VK_NEXT, true)),
        "INSERT" | "INS" => Some((VK_INSERT, true)),
        _ => None,
    };
    if let Some(x) = simple {
        return Some((x.0 as u16, x.1));
    }
    if let Some(n) = name.strip_prefix('F') {
        if let Ok(num) = n.parse::<u16>() {
            if (1..=24).contains(&num) {
                return Some((0x6F + num, false));
            }
        }
    }
    let mut it = name.chars();
    if let (Some(c), None) = (it.next(), it.next()) {
        if c.is_ascii_digit() {
            return Some((c as u16, false));
        }
        let up = c.to_ascii_uppercase();
        if up.is_ascii_uppercase() {
            return Some((up as u16, false));
        }
    }
    None
}

struct Rng(u64);

impl Rng {
    fn new() -> Rng {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| (d.subsec_nanos() as u64) ^ (d.as_secs() << 3))
            .unwrap_or(12345);
        Rng(seed | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        if hi <= lo {
            return lo;
        }
        lo + self.next() % (hi - lo)
    }
}

fn char_delay(mode: TypingMode, rng: &mut Rng) -> u64 {
    match mode {
        TypingMode::Instant => 0,
        TypingMode::Fast => rng.range(2, 7),
        TypingMode::Human => rng.range(12, 42),
        TypingMode::Slow => rng.range(60, 130),
    }
}

fn key_gap(mode: TypingMode, rng: &mut Rng) -> u64 {
    match mode {
        TypingMode::Instant => 10,
        TypingMode::Fast => rng.range(20, 40),
        TypingMode::Human => rng.range(50, 130),
        TypingMode::Slow => rng.range(120, 220),
    }
}

pub fn run_job(job: Job) {
    let started = std::time::Instant::now();
    if job.delay_ms > 0 {
        std::thread::sleep(Duration::from_millis(job.delay_ms));
    }
    if let Some(t) = job.target {
        unsafe {
            SetForegroundWindow(t.0);
        }
        std::thread::sleep(Duration::from_millis(120));
    }
    let items = tokenize(&job.text);
    if job.mode == TypingMode::Instant {
        run_instant(&items, job.delete);
    } else {
        if job.delete > 0 {
            backspaces(job.delete);
            std::thread::sleep(Duration::from_millis(35));
        }
        let mut rng = Rng::new();
        for item in &items {
            match item {
                Item::Text(s) => {
                    for ch in s.encode_utf16() {
                        type_char(ch);
                        let d = char_delay(job.mode, &mut rng);
                        if d > 0 {
                            std::thread::sleep(Duration::from_millis(d));
                        }
                    }
                }
                Item::Key { vk, mods, extended } => {
                    if mods.is_empty() {
                        tap_vk(*vk, *extended);
                    } else {
                        combo(mods, *vk, *extended);
                    }
                    let d = key_gap(job.mode, &mut rng);
                    if d > 0 {
                        std::thread::sleep(Duration::from_millis(d));
                    }
                }
                Item::Click { down, up } => {
                    let ev = [mouse_input(*down), mouse_input(*up)];
                    send(&ev);
                    std::thread::sleep(Duration::from_millis(30));
                }
                Item::Sleep(ms) => std::thread::sleep(Duration::from_millis(*ms)),
            }
        }
    }
    let elapsed = started.elapsed().as_millis() as u64;
    write_last_expansion(job.mode, job.delete, job.text.chars().count(), elapsed);
    if elapsed > 120 {
        logline(&format!(
            "slow expansion: mode={:?} deleted={} chars={} ms={}",
            job.mode,
            job.delete,
            job.text.chars().count(),
            elapsed
        ));
    }
}

fn run_instant(items: &[Item], delete: usize) {
    let mut ev: Vec<INPUT> = Vec::with_capacity(96);
    for _ in 0..delete {
        ev.push(vk_input(VK_BACK as u16, false, false));
        ev.push(vk_input(VK_BACK as u16, true, false));
    }
    for item in items {
        match item {
            Item::Text(s) => {
                let short = s.chars().count() < 3;
                let pasted = if short {
                    false
                } else if let Some(f) = PASTE_FN.get() {
                    send(&ev);
                    ev.clear();
                    f(s)
                } else {
                    false
                };
                if !pasted {
                    for u in s.encode_utf16() {
                        ev.push(unicode_input(u, false));
                        ev.push(unicode_input(u, true));
                    }
                }
            }
            Item::Key { vk, mods, extended } => {
                for m in mods {
                    ev.push(vk_input(*m, false, false));
                }
                ev.push(vk_input(*vk, false, *extended));
                ev.push(vk_input(*vk, true, *extended));
                for m in mods.iter().rev() {
                    ev.push(vk_input(*m, true, false));
                }
            }
            Item::Click { down, up } => {
                ev.push(mouse_input(*down));
                ev.push(mouse_input(*up));
            }
            Item::Sleep(ms) => {
                send(&ev);
                ev.clear();
                std::thread::sleep(Duration::from_millis(*ms));
            }
        }
        if ev.len() >= 240 {
            send(&ev);
            ev.clear();
        }
    }
    send(&ev);
}

fn write_last_expansion(mode: TypingMode, deleted: usize, chars: usize, ms: u64) {
    let dir = crate::config::data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let line = format!(
        "mode={:?} deleted={} chars={} ms={}",
        mode, deleted, chars, ms
    );
    let _ = std::fs::write(dir.join("last-expansion.txt"), line);
}
