use crate::config::{Config, Snippet, TriggerMode, TypingMode};
use crate::exec;
use crate::win::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Instant;

pub const GAP_RESET_MS: u64 = 2500;
pub const CONFIRM_TIMEOUT_MS: u128 = 2500;

#[derive(Clone)]
pub enum UiCmd {
    TogglePalette,
    ConfirmShow(String),
    ConfirmHide,
}

pub struct Job {
    pub delete: usize,
    pub text: String,
    pub mode: TypingMode,
    pub target: Option<exec::SendHwnd>,
    pub delay_ms: u64,
}

pub struct Shared {
    pub map: HashMap<String, Snippet>,
    pub exact: HashMap<String, Snippet>,
    pub max_len: usize,
    pub paused: bool,
    pub default_mode: TypingMode,
}

#[derive(Clone)]
pub struct Engine {
    pub shared: Arc<RwLock<Shared>>,
    pub job_tx: Sender<Job>,
}

struct HookCtx {
    shared: Arc<RwLock<Shared>>,
    job_tx: Mutex<Sender<Job>>,
    ui: Box<dyn Fn(UiCmd) + Send + Sync>,
}

struct Pending {
    snippet: Snippet,
    delete: usize,
    mode: TypingMode,
    at: Instant,
}

static HOOK_CTX: OnceLock<HookCtx> = OnceLock::new();

thread_local! {
    static TAIL: RefCell<Vec<char>> = RefCell::new(Vec::new());
    static SWALLOW_SPACE_UP: Cell<bool> = Cell::new(false);
    static LAST_KEY_MS: Cell<u64> = Cell::new(0);
    static LAST_FG: Cell<isize> = Cell::new(0);
    static PENDING: RefCell<Option<Pending>> = RefCell::new(None);
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl Engine {
    pub fn new() -> Engine {
        let (tx, rx) = std::sync::mpsc::channel::<Job>();
        std::thread::spawn(move || {
            while let Ok(job) = rx.recv() {
                exec::run_job(job);
            }
        });
        Engine {
            shared: Arc::new(RwLock::new(Shared {
                map: HashMap::new(),
                exact: HashMap::new(),
                max_len: 8,
                paused: false,
                default_mode: TypingMode::Fast,
            })),
            job_tx: tx,
        }
    }

    pub fn update(&self, cfg: &Config) {
        let mut map = HashMap::new();
        let mut exact = HashMap::new();
        let mut max_len = 4usize;
        for s in &cfg.snippets {
            if s.trigger.is_empty() {
                continue;
            }
            let len = s.trigger.chars().count();
            if len > max_len {
                max_len = len;
            }
            if s.case_sensitive {
                exact.insert(s.trigger.clone(), s.clone());
            } else {
                map.insert(s.trigger.to_lowercase(), s.clone());
            }
        }
        if let Ok(mut sh) = self.shared.write() {
            sh.map = map;
            sh.exact = exact;
            sh.max_len = max_len;
            sh.paused = cfg.settings.paused;
            sh.default_mode = cfg.settings.default_mode;
        }
    }

    pub fn set_paused(&self, paused: bool) {
        if let Ok(mut sh) = self.shared.write() {
            sh.paused = paused;
        }
    }

    pub fn start_hook(&self, ui: Box<dyn Fn(UiCmd) + Send + Sync>) {
        let ctx = HookCtx {
            shared: self.shared.clone(),
            job_tx: Mutex::new(self.job_tx.clone()),
            ui,
        };
        let _ = HOOK_CTX.set(ctx);
        std::thread::spawn(|| unsafe {
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), std::ptr::null_mut(), 0);
            if hook.is_null() {
                exec::logline("hook install failed");
                return;
            }
            exec::logline("hook installed");
            let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), std::ptr::null_mut(), 0);
            if mouse.is_null() {
                exec::logline("mouse hook install failed");
            }
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}
            UnhookWindowsHookEx(hook);
            if !mouse.is_null() {
                UnhookWindowsHookEx(mouse);
            }
        });
    }
}

fn is_palette_hotkey() -> bool {
    let ctrl = key_down(VK_CONTROL);
    let alt = key_down(VK_MENU);
    let shift = key_down(VK_SHIFT);
    let win = key_down(VK_LWIN) || key_down(VK_RWIN);
    ctrl && !alt && !shift && !win
}

fn cancel_pending(ctx: &HookCtx) {
    let had = PENDING.with(|p| p.borrow_mut().take().is_some());
    if had {
        (ctx.ui)(UiCmd::ConfirmHide);
    }
}

fn note_key_context(ctx: &HookCtx) {
    let now = now_ms();
    let last = LAST_KEY_MS.replace(now);
    if now.saturating_sub(last) > GAP_RESET_MS {
        TAIL.with(|t| t.borrow_mut().clear());
    }
    let fg = unsafe { GetForegroundWindow() } as isize;
    if LAST_FG.replace(fg) != fg {
        TAIL.with(|t| t.borrow_mut().clear());
        cancel_pending(ctx);
    }
}

fn enqueue(ctx: &HookCtx, snip: Snippet, count: usize, mode: TypingMode) {
    exec::logline(&format!(
        "expand '{}' delete={} out={}",
        snip.trigger,
        count,
        snip.replacement.chars().count()
    ));
    if let Ok(tx) = ctx.job_tx.lock() {
        let _ = tx.send(Job {
            delete: count,
            text: snip.replacement.clone(),
            mode: snip.mode.unwrap_or(mode),
            target: None,
            delay_ms: 0,
        });
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let msg = wparam as u32;
        let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
        if down || up {
            let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
            if kb.dwExtraInfo != exec::MAGIC {
                if let Some(ctx) = HOOK_CTX.get() {
                    if handle_key(ctx, kb, down) {
                        return 1;
                    }
                }
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let msg = wparam as u32;
        if msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN || msg == WM_MBUTTONDOWN || msg == WM_XBUTTONDOWN {
            TAIL.with(|t| t.borrow_mut().clear());
            if let Some(ctx) = HOOK_CTX.get() {
                cancel_pending(ctx);
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

unsafe fn handle_key(ctx: &HookCtx, kb: &KBDLLHOOKSTRUCT, down: bool) -> bool {
    let vk = kb.vkCode;

    if !down {
        if vk == VK_SPACE && SWALLOW_SPACE_UP.with(|s| s.replace(false)) {
            return true;
        }
        note_key_context(ctx);
        return false;
    }

    if vk == VK_SPACE && is_palette_hotkey() {
        cancel_pending(ctx);
        (ctx.ui)(UiCmd::TogglePalette);
        SWALLOW_SPACE_UP.with(|s| s.set(true));
        return true;
    }

    let pending = PENDING.with(|p| p.borrow_mut().take());
    if let Some(pend) = pending {
        (ctx.ui)(UiCmd::ConfirmHide);
        TAIL.with(|t| t.borrow_mut().clear());
        if pend.at.elapsed().as_millis() <= CONFIRM_TIMEOUT_MS {
            if vk == VK_RETURN || vk == VK_TAB {
                exec::logline(&format!("confirm accepted: '{}'", pend.snippet.trigger));
                if let Ok(tx) = ctx.job_tx.lock() {
                    let _ = tx.send(Job {
                        delete: pend.delete,
                        text: pend.snippet.replacement.clone(),
                        mode: pend.snippet.mode.unwrap_or(pend.mode),
                        target: None,
                        delay_ms: 0,
                    });
                }
                return true;
            }
            if vk == VK_ESCAPE {
                exec::logline(&format!("confirm cancelled: '{}'", pend.snippet.trigger));
                return true;
            }
            exec::logline(&format!("confirm dismissed: '{}'", pend.snippet.trigger));
        } else {
            exec::logline(&format!("confirm expired: '{}'", pend.snippet.trigger));
        }
    }

    note_key_context(ctx);

    match vk {
        VK_BACK => {
            TAIL.with(|t| {
                t.borrow_mut().pop();
            });
            return false;
        }
        VK_ESCAPE | VK_RETURN | VK_TAB | VK_UP | VK_DOWN | VK_LEFT | VK_RIGHT | VK_HOME | VK_END | VK_PRIOR
        | VK_NEXT | VK_DELETE => {
            TAIL.with(|t| {
                t.borrow_mut().clear();
            });
            return false;
        }
        _ => {}
    }

    let ctrl = key_down(VK_CONTROL);
    let alt = key_down(VK_MENU);
    let win = key_down(VK_LWIN) || key_down(VK_RWIN);
    if ctrl || alt || win {
        return false;
    }

    let ch = match decode_char(vk) {
        Some(c) => c,
        None => return false,
    };

    let mut matched: Option<(Snippet, usize, TypingMode)> = None;
    TAIL.with(|t| {
        let mut tail = t.borrow_mut();
        if tail.len() >= 64 {
            tail.remove(0);
        }
        tail.push(ch);
        if let Ok(sh) = ctx.shared.read() {
            if !sh.paused && !exec::own_foreground() {
                matched = find_match(&sh, &tail);
            }
        }
        if matched.is_some() {
            tail.clear();
        }
    });

    if let Some((snip, count, mode)) = matched {
        match snip.trigger_mode {
            TriggerMode::Manual => {
                exec::logline(&format!("manual-only trigger typed: '{}'", snip.trigger));
            }
            TriggerMode::Confirm => {
                exec::logline(&format!("confirm requested: '{}'", snip.trigger));
                PENDING.with(|p| {
                    *p.borrow_mut() = Some(Pending {
                        snippet: snip.clone(),
                        delete: count,
                        mode,
                        at: Instant::now(),
                    });
                });
                (ctx.ui)(UiCmd::ConfirmShow(snip.trigger.clone()));
            }
            TriggerMode::Auto => {
                enqueue(ctx, snip, count, mode);
            }
        }
    }
    false
}

fn find_match(sh: &Shared, tail: &[char]) -> Option<(Snippet, usize, TypingMode)> {
    let n = tail.len();
    let max = sh.max_len.min(n);
    for k in (1..=max).rev() {
        let slice: String = tail[n - k..].iter().collect();
        let found = sh
            .exact
            .get(&slice)
            .cloned()
            .or_else(|| sh.map.get(&slice.to_lowercase()).cloned());
        let s = match found {
            Some(s) => s,
            None => continue,
        };
        if s.word_boundary && k < n && tail[n - k - 1].is_alphanumeric() {
            continue;
        }
        return Some((s, k, sh.default_mode));
    }
    None
}

unsafe fn decode_char(vk: u32) -> Option<char> {
    let shift = key_down(VK_SHIFT);
    let caps = (GetKeyState(VK_CAPITAL as i32) as u16 & 1) != 0;
    match vk {
        0x30..=0x39 => char::from_u32(vk),
        0x41..=0x5A => {
            let upper = shift ^ caps;
            let c = char::from_u32(vk)?;
            Some(if upper { c } else { c.to_ascii_lowercase() })
        }
        VK_SPACE => Some(' '),
        _ => {
            let r = MapVirtualKeyW(vk, MAPVK_VK_TO_CHAR);
            if r == 0 {
                return None;
            }
            let base = (r & 0xFF) as u8 as char;
            if !base.is_ascii_graphic() {
                return None;
            }
            Some(if shift { shift_char(base) } else { base })
        }
    }
}

fn shift_char(c: char) -> char {
    match c {
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '0' => ')',
        '-' => '_',
        '=' => '+',
        '[' => '{',
        ']' => '}',
        '\\' => '|',
        ';' => ':',
        '\'' => '"',
        ',' => '<',
        '.' => '>',
        '/' => '?',
        '~' => '~',
        other => other,
    }
}

fn key_down(vk: u32) -> bool {
    unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
}
