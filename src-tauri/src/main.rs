#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod engine;
mod exec;
mod fdb;
mod win;

use config::{Config, Settings, Snippet, TypingMode};
use engine::{Engine, Job, UiCmd};
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};

static PALETTE_SHOWN_MS: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

struct AppState {
    config: std::sync::Mutex<Config>,
    engine: Engine,
    prev_hwnd: std::sync::Mutex<Option<exec::SendHwnd>>,
    pause_item: std::sync::Mutex<Option<CheckMenuItem<tauri::Wry>>>,
    autostart_item: std::sync::Mutex<Option<CheckMenuItem<tauri::Wry>>>,
}

#[derive(Serialize, Clone)]
struct Status {
    paused: bool,
    autostart: bool,
    default_mode: TypingMode,
    snippet_count: usize,
    version: String,
    fastkeys_db: Option<String>,
}

#[derive(Serialize, Clone)]
struct ImportRule {
    trigger: String,
    name: String,
    preview: String,
}

#[derive(Serialize, Clone)]
struct ImportReport {
    total_lines: usize,
    parsed: usize,
    imported: usize,
    skipped_duplicates: usize,
    rules: Vec<ImportRule>,
}

fn status_of(app: &AppHandle) -> Status {
    let st = app.state::<AppState>();
    let cfg = st.config.lock().unwrap();
    Status {
        paused: cfg.settings.paused,
        autostart: cfg.settings.autostart,
        default_mode: cfg.settings.default_mode,
        snippet_count: cfg.snippets.len(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        fastkeys_db: fdb::detect_path(),
    }
}

fn toggle_palette(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("palette") {
        let visible = w.is_visible().unwrap_or(false);
        if visible {
            let _ = w.hide();
            exec::logline("palette hidden");
            restore_focus(app);
        } else {
            let fg = exec::capture_foreground();
            {
                let st = app.state::<AppState>();
                let guard = st.prev_hwnd.lock();
                if let Ok(mut p) = guard {
                    *p = fg;
                }
            }
            PALETTE_SHOWN_MS.store(now_ms(), Ordering::Relaxed);
            let _ = w.center();
            let _ = w.show();
            let _ = w.set_focus();
            exec::logline("palette shown");
            let _ = app.emit("palette-opened", ());
        }
    }
}

fn restore_focus(app: &AppHandle) {
    let target = {
        let st = app.state::<AppState>();
        let guard = st.prev_hwnd.lock().ok();
        guard.and_then(|mut g| g.take())
    };
    if let Some(h) = target {
        std::thread::sleep(std::time::Duration::from_millis(70));
        unsafe {
            win::SetForegroundWindow(h.0);
        }
    }
}

fn open_manager_window(app: &AppHandle, tab: Option<String>) {
    if let Some(w) = app.get_webview_window("manager") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        let _ = app.emit("manager-tab", tab.unwrap_or_default());
    }
}

static CONFIRM_GEN: AtomicU64 = AtomicU64::new(0);
static CONFIRM_TEXT: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

fn show_confirm(app: &AppHandle, trigger: &str) {
    let gen = CONFIRM_GEN.fetch_add(1, Ordering::Relaxed) + 1;
    if let Ok(mut t) = CONFIRM_TEXT.lock() {
        *t = trigger.to_string();
    }
    if let Some(w) = app.get_webview_window("confirm") {
        if let Ok(Some(mon)) = app.primary_monitor() {
            let size = mon.size();
            let scale = mon.scale_factor();
            let (win_w, win_h) = w
                .outer_size()
                .map(|s| (s.width as i32, s.height as i32))
                .unwrap_or((420, 76));
            let x = ((size.width as i32 - win_w) / 2).max(0);
            let y = (size.height as i32 - win_h - (150.0 * scale) as i32).max(0);
            let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
        }
        let _ = w.show();
        let _ = app.emit("confirm-show", trigger.to_string());
        let late = app.clone();
        let late_text = trigger.to_string();
        std::thread::spawn(move || {
            for delay in [250u64, 600, 1200] {
                std::thread::sleep(std::time::Duration::from_millis(delay));
                let _ = late.emit("confirm-show", late_text.clone());
            }
        });
    }
    let h = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(2600));
        if CONFIRM_GEN.load(Ordering::Relaxed) == gen {
            if let Some(w) = h.get_webview_window("confirm") {
                let _ = w.hide();
            }
        }
    });
}

fn hide_confirm(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("confirm") {
        let _ = w.hide();
    }
}

#[tauri::command]
fn confirm_text() -> String {
    CONFIRM_TEXT.lock().map(|t| t.clone()).unwrap_or_default()
}

fn apply_autostart(enabled: bool) -> Result<(), String> {
    use std::process::Command;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let key = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    let mut cmd = Command::new("reg.exe");
    if enabled {
        cmd.arg("add")
            .arg(key)
            .arg("/v")
            .arg("KeyFlow")
            .arg("/t")
            .arg("REG_SZ")
            .arg("/d")
            .arg(format!("\"{}\" --autostart", exe.display()))
            .arg("/f");
    } else {
        cmd.arg("delete").arg(key).arg("/v").arg("KeyFlow").arg("/f");
    }
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    if enabled && !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(())
}

fn sync_tray_check(app: &AppHandle) {
    let st = app.state::<AppState>();
    let (paused, autostart) = {
        let c = st.config.lock().unwrap();
        (c.settings.paused, c.settings.autostart)
    };
    {
        let guard = st.pause_item.lock().unwrap();
        if let Some(item) = guard.as_ref() {
            let _ = item.set_checked(paused);
        }
    }
    {
        let guard = st.autostart_item.lock().unwrap();
        if let Some(item) = guard.as_ref() {
            let _ = item.set_checked(autostart);
        }
    }
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let (paused, autostart) = {
        let st = app.state::<AppState>();
        let c = st.config.lock().unwrap();
        (c.settings.paused, c.settings.autostart)
    };
    let palette_item = MenuItem::with_id(app, "palette", "Open palette  (Ctrl+Space)", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let pause_item = CheckMenuItem::with_id(app, "pause", "Pause expansion", true, paused, None::<&str>)?;
    let autostart_item = CheckMenuItem::with_id(app, "autostart", "Start with Windows", true, autostart, None::<&str>)?;
    let reload_item = MenuItem::with_id(app, "reload", "Reload configuration", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Exit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&palette_item, &settings_item, &pause_item, &autostart_item, &reload_item, &quit_item],
    )?;
    {
        let st = app.state::<AppState>();
        *st.pause_item.lock().unwrap() = Some(pause_item.clone());
        *st.autostart_item.lock().unwrap() = Some(autostart_item.clone());
    }

    let _tray = TrayIconBuilder::with_id("keyflow-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("KeyFlow - text expander")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "palette" => toggle_palette(app),
            "settings" => open_manager_window(app, None),
            "pause" => {
                let now = {
                    let st = app.state::<AppState>();
                    let c = st.config.lock().unwrap();
                    !c.settings.paused
                };
                let _ = set_paused(app.clone(), app.state::<AppState>(), now);
            }
            "autostart" => {
                let now = {
                    let st = app.state::<AppState>();
                    let c = st.config.lock().unwrap();
                    !c.settings.autostart
                };
                let _ = set_autostart(app.clone(), app.state::<AppState>(), now);
            }
            "reload" => {
                let _ = reload_config(app.clone(), app.state::<AppState>());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                open_manager_window(tray.app_handle(), None);
            }
        })
        .build(app)?;
    Ok(())
}

fn report_from(text: &str, rules: &[fdb::FdbRule], imported: usize, skipped: usize) -> ImportReport {
    ImportReport {
        total_lines: text.lines().count(),
        parsed: rules.len(),
        imported,
        skipped_duplicates: skipped,
        rules: rules
            .iter()
            .map(|r| {
                let flat: String = r
                    .replacement
                    .chars()
                    .map(|c| if c == '\n' { '|' } else { c })
                    .collect();
                let flat = flat.replace('\t', "->");
                let preview = if flat.chars().count() > 60 {
                    flat.chars().take(60).collect::<String>() + "..."
                } else {
                    flat
                };
                ImportRule {
                    trigger: r.trigger.clone(),
                    name: r.name.clone(),
                    preview,
                }
            })
            .collect(),
    }
}

#[tauri::command]
fn list_snippets(state: tauri::State<AppState>) -> Vec<Snippet> {
    state.config.lock().unwrap().snippets.clone()
}

#[tauri::command]
fn save_snippet(app: AppHandle, state: tauri::State<AppState>, snippet: Snippet) -> Result<Vec<Snippet>, String> {
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    let mut snip = snippet;
    if snip.id.trim().is_empty() {
        snip.id = config::new_id();
    }
    if let Some(existing) = cfg.snippets.iter_mut().find(|s| s.id == snip.id) {
        *existing = snip;
    } else {
        cfg.snippets.push(snip);
    }
    cfg.save().map_err(|e| e.to_string())?;
    let list = cfg.snippets.clone();
    state.engine.update(&cfg);
    drop(cfg);
    let _ = app.emit("config-updated", ());
    Ok(list)
}

#[tauri::command]
fn delete_snippet(app: AppHandle, state: tauri::State<AppState>, id: String) -> Result<Vec<Snippet>, String> {
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    cfg.snippets.retain(|s| s.id != id);
    cfg.save().map_err(|e| e.to_string())?;
    let list = cfg.snippets.clone();
    state.engine.update(&cfg);
    drop(cfg);
    let _ = app.emit("config-updated", ());
    Ok(list)
}

#[tauri::command]
fn get_status(app: AppHandle) -> Status {
    status_of(&app)
}

#[tauri::command]
fn update_settings(app: AppHandle, state: tauri::State<AppState>, settings: Settings) -> Result<Status, String> {
    let (prev_autostart, new_autostart) = {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        let prev = cfg.settings.autostart;
        cfg.settings = settings;
        cfg.save().map_err(|e| e.to_string())?;
        state.engine.update(&cfg);
        (prev, cfg.settings.autostart)
    };
    if prev_autostart != new_autostart {
        let _ = apply_autostart(new_autostart);
    }
    sync_tray_check(&app);
    Ok(status_of(&app))
}

#[tauri::command]
fn set_paused(app: AppHandle, state: tauri::State<AppState>, paused: bool) -> Result<Status, String> {
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.settings.paused = paused;
        cfg.save().map_err(|e| e.to_string())?;
    }
    state.engine.set_paused(paused);
    sync_tray_check(&app);
    Ok(status_of(&app))
}

#[tauri::command]
fn set_autostart(app: AppHandle, state: tauri::State<AppState>, autostart: bool) -> Result<Status, String> {
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.settings.autostart = autostart;
        cfg.save().map_err(|e| e.to_string())?;
    }
    apply_autostart(autostart)?;
    sync_tray_check(&app);
    Ok(status_of(&app))
}

#[tauri::command]
fn reload_config(app: AppHandle, state: tauri::State<AppState>) -> Result<Status, String> {
    let fresh = Config::load();
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        *cfg = fresh;
        state.engine.update(&cfg);
    }
    sync_tray_check(&app);
    Ok(status_of(&app))
}

#[tauri::command]
fn hide_palette(app: AppHandle) {
    if let Some(w) = app.get_webview_window("palette") {
        let _ = w.hide();
    }
    restore_focus(&app);
}

#[tauri::command]
fn open_manager(app: AppHandle, tab: Option<String>) {
    open_manager_window(&app, tab);
}

#[tauri::command]
fn open_manager_snippet(app: AppHandle, mode: String, snippet: Snippet) {
    open_manager_window(&app, Some("snippets".to_string()));
    let _ = app.emit(
        "manager-open-snippet",
        serde_json::json!({ "mode": mode, "snippet": snippet }),
    );
}

#[tauri::command]
fn execute_snippet(app: AppHandle, state: tauri::State<AppState>, id: String) -> Result<(), String> {
    let (text, mode) = {
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        let snip = cfg.snippets.iter().find(|s| s.id == id).ok_or("snippet not found")?;
        (snip.replacement.clone(), snip.mode.unwrap_or(cfg.settings.default_mode))
    };
    if let Some(w) = app.get_webview_window("palette") {
        let _ = w.hide();
    }
    let target = {
        let mut guard = state.prev_hwnd.lock().map_err(|e| e.to_string())?;
        guard.take()
    };
    if let Some(h) = target {
        std::thread::sleep(std::time::Duration::from_millis(60));
        unsafe {
            win::SetForegroundWindow(h.0);
        }
    }
    let _ = state.engine.job_tx.send(Job {
        delete: 0,
        text,
        mode,
        target: None,
        delay_ms: 80,
    });
    Ok(())
}

#[tauri::command]
fn copy_snippet(app: AppHandle, state: tauri::State<AppState>, id: String) -> Result<(), String> {
    let text = {
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.snippets
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.replacement.clone())
            .ok_or("snippet not found")?
    };
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().write_text(text).map_err(|e| e.to_string())?;
    if let Some(w) = app.get_webview_window("palette") {
        let _ = w.hide();
    }
    restore_focus(&app);
    Ok(())
}

#[tauri::command]
fn copy_text(app: AppHandle, text: String) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

#[tauri::command]
fn send_test(state: tauri::State<AppState>, text: String, mode: TypingMode, delay_ms: u64) -> Result<(), String> {
    let _ = state.engine.job_tx.send(Job {
        delete: 0,
        text,
        mode,
        target: None,
        delay_ms,
    });
    Ok(())
}

#[tauri::command]
fn fastkeys_db_path() -> Option<String> {
    fdb::detect_path()
}

#[tauri::command]
fn parse_fdb(path: String) -> Result<ImportReport, String> {
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read failed: {}", e))?;
    let rules = fdb::parse(&text);
    Ok(report_from(&text, &rules, 0, 0))
}

#[tauri::command]
fn import_fdb(app: AppHandle, state: tauri::State<AppState>, path: String, replace: bool) -> Result<ImportReport, String> {
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read failed: {}", e))?;
    let rules = fdb::parse(&text);
    let new_snippets = fdb::to_snippets(&rules, "fk-");
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    let mut imported = 0usize;
    let mut skipped = 0usize;
    for s in new_snippets {
        let key = s.trigger.to_lowercase();
        match cfg.snippets.iter().position(|x| x.trigger.to_lowercase() == key) {
            Some(pos) => {
                if replace {
                    cfg.snippets[pos] = s;
                    imported += 1;
                } else {
                    skipped += 1;
                }
            }
            None => {
                cfg.snippets.push(s);
                imported += 1;
            }
        }
    }
    cfg.save().map_err(|e| e.to_string())?;
    state.engine.update(&cfg);
    drop(cfg);
    let _ = app.emit("config-updated", ());
    Ok(report_from(&text, &rules, imported, skipped))
}

fn run_cli_import(path: &str) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read failed: {}", e))?;
    let rules = fdb::parse(&text);
    let mut cfg = Config::load();
    let new_snippets = fdb::to_snippets(&rules, "fk-");
    let mut imported = 0usize;
    for s in new_snippets {
        let key = s.trigger.to_lowercase();
        match cfg.snippets.iter().position(|x| x.trigger.to_lowercase() == key) {
            Some(pos) => cfg.snippets[pos] = s,
            None => cfg.snippets.push(s),
        }
        imported += 1;
    }
    cfg.save().map_err(|e| e.to_string())?;
    let report = format!(
        "imported {} snippets from {} ({} rules parsed, {} total snippets)",
        imported,
        path,
        rules.len(),
        cfg.snippets.len()
    );
    let dir = config::data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("last-import.txt"), &report);
    Ok(report)
}

fn spawn_config_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let path = config::config_path();
        let mut last = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(2000));
            let now = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            if now.is_none() || now == last {
                continue;
            }
            last = now;
            std::thread::sleep(std::time::Duration::from_millis(150));
            let fresh = std::fs::read_to_string(&path)
                .ok()
                .and_then(|t| serde_json::from_str::<Config>(&t).ok());
            let fresh = match fresh {
                Some(f) => f,
                None => continue,
            };
            let st = app.state::<AppState>();
            let changed = {
                let cfg = st.config.lock().unwrap();
                cfg.snippets != fresh.snippets || cfg.settings != fresh.settings
            };
            if changed {
                if let Ok(mut cfg) = st.config.lock() {
                    *cfg = fresh;
                    st.engine.update(&cfg);
                }
                let _ = app.emit("config-updated", ());
                exec::logline("config reloaded from disk (external change)");
            }
        }
    });
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--import-fdb") {
        if let Some(path) = args.get(pos + 1) {
            match run_cli_import(path) {
                Ok(msg) => println!("{}", msg),
                Err(e) => {
                    eprintln!("import error: {}", e);
                    std::process::exit(2);
                }
            }
            return;
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            open_manager_window(app, None);
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            list_snippets,
            save_snippet,
            delete_snippet,
            get_status,
            update_settings,
            set_paused,
            set_autostart,
            reload_config,
            hide_palette,
            open_manager,
            open_manager_snippet,
            confirm_text,
            execute_snippet,
            copy_snippet,
            copy_text,
            send_test,
            fastkeys_db_path,
            parse_fdb,
            import_fdb
        ])
        .setup(|app| {
            let cfg = Config::load();
            let engine = Engine::new();
            engine.update(&cfg);

            {
                let app_handle = app.handle().clone();
                exec::set_paste_fn(Box::new(move |text: &str| -> bool {
                    use tauri_plugin_clipboard_manager::ClipboardExt;
                    let cb = app_handle.clipboard();
                    let previous = cb.read_text().ok();
                    if cb.write_text(text.to_string()).is_err() {
                        return false;
                    }
                    exec::combo(&[win::VK_CONTROL as u16], 0x56, false);
                    let h = app_handle.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        use tauri_plugin_clipboard_manager::ClipboardExt;
                        let cb = h.clipboard();
                        match previous {
                            Some(prev) => {
                                let _ = cb.write_text(prev);
                            }
                            None => {
                                let _ = cb.clear();
                            }
                        }
                    });
                    true
                }));
            }

            let handle_for_ui = app.handle().clone();
            let ui: Box<dyn Fn(UiCmd) + Send + Sync> = Box::new(move |cmd| {
                let h = handle_for_ui.clone();
                let h2 = h.clone();
                let _ = h.run_on_main_thread(move || match cmd {
                    UiCmd::TogglePalette => toggle_palette(&h2),
                    UiCmd::ConfirmShow(trigger) => show_confirm(&h2, &trigger),
                    UiCmd::ConfirmHide => hide_confirm(&h2),
                });
            });
            engine.start_hook(ui);

            app.manage(AppState {
                config: std::sync::Mutex::new(cfg),
                engine,
                prev_hwnd: std::sync::Mutex::new(None),
                pause_item: std::sync::Mutex::new(None),
                autostart_item: std::sync::Mutex::new(None),
            });

            setup_tray(app.handle())?;
            {
                let st = app.state::<AppState>();
                let c = st.config.lock().unwrap();
                exec::logline(&format!(
                    "KeyFlow {} started with {} snippets",
                    env!("CARGO_PKG_VERSION"),
                    c.snippets.len()
                ));
            }
            spawn_config_watcher(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                if window.label() == "manager" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            WindowEvent::Focused(false) => {
                if window.label() == "palette" && now_ms().saturating_sub(PALETTE_SHOWN_MS.load(Ordering::Relaxed)) > 400 {
                    let _ = window.hide();
                }
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running KeyFlow");
}
