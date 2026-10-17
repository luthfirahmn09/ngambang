use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;

use objc2::runtime::{AnyClass, AnyObject};
use objc2::{define_class, ClassType, MainThreadOnly};
use objc2_app_kit::{NSPanel, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask};

extern "C" {
    fn object_setClass(obj: *mut AnyObject, cls: *const AnyClass) -> *const AnyClass;
}

define_class!(
    /// NSPanel that can still take keyboard focus even though it is borderless/non-activating.
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "NgambangPanel"]
    struct FloatingPanel;

    impl FloatingPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key(&self) -> bool { true }
        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main(&self) -> bool { false }
    }
);
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{ActivationPolicy, AppHandle, Emitter, LogicalSize, Manager, State, WebviewWindow};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[derive(Default)]
struct Chat {
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
    /// Size to restore when expanding from the collapsed bubble.
    expanded: Mutex<Option<LogicalSize<f64>>>,
}

fn find_claude() -> Option<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let candidates = [
        "/opt/homebrew/bin/claude".to_string(),
        "/usr/local/bin/claude".to_string(),
        format!("{home}/.local/bin/claude"),
        format!("{home}/.claude/local/claude"),
    ];
    if let Some(p) = candidates.iter().find(|p| std::path::Path::new(p).exists()) {
        return Some(p.clone());
    }
    let out = Command::new("/bin/zsh").args(["-lc", "command -v claude"]).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// Starts one long-lived `claude -p` process; it is reused for every turn so startup cost is paid once.
fn spawn_claude(app: &AppHandle, state: &Chat) -> Result<(), String> {
    let claude = find_claude().ok_or("Claude CLI tidak ketemu. Install Claude Code dulu.")?;
    let mut cmd = Command::new(claude);
    cmd.args([
        "-p", "--input-format", "stream-json", "--output-format", "stream-json",
        "--verbose", "--include-partial-messages",
    ]);
    let path = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into());
    cmd.env("PATH", format!("/opt/homebrew/bin:/usr/local/bin:{path}"))
        .current_dir(std::env::var("HOME").unwrap_or_else(|_| "/".into()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    *state.stdin.lock().unwrap() = child.stdin.take();
    *state.child.lock().unwrap() = Some(child);

    let app2 = app.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = app2.emit("claude-line", line);
        }
        // Process ended (crash, stop or new chat).
        let err: String = BufReader::new(stderr).lines().map_while(Result::ok).collect::<Vec<_>>().join("\n");
        let st = app2.state::<Chat>();
        st.stdin.lock().unwrap().take();
        let code = st.child.lock().unwrap().take().and_then(|mut c| c.wait().ok()).and_then(|s| s.code());
        let _ = app2.emit("claude-done", serde_json::json!({ "code": code, "stderr": err }));
    });
    Ok(())
}

#[tauri::command]
fn send(app: AppHandle, state: State<Chat>, prompt: String, images: Vec<String>) -> Result<(), String> {
    if state.stdin.lock().unwrap().is_none() {
        spawn_claude(&app, &state)?;
    }
    let mut content: Vec<serde_json::Value> = images
        .into_iter()
        .map(|data| serde_json::json!({ "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": data } }))
        .collect();
    content.push(serde_json::json!({ "type": "text", "text": prompt }));
    let msg = serde_json::json!({ "type": "user", "message": { "role": "user", "content": content } });
    let mut guard = state.stdin.lock().unwrap();
    let stdin = guard.as_mut().ok_or("Claude tidak jalan")?;
    writeln!(stdin, "{msg}").and_then(|_| stdin.flush()).map_err(|e| e.to_string())
}

/// Screenshots the screen (`interactive` = drag to select a region) and returns it as base64 PNG.
/// The floating window is hidden during capture so it doesn't end up in the shot.
/// Needs the Screen Recording permission for the app the first time.
#[tauri::command]
async fn capture(window: WebviewWindow, interactive: bool) -> Result<String, String> {
    use base64::Engine;
    let path = std::env::temp_dir().join(format!("ngambang-{}.png", std::process::id()));
    window.hide().map_err(|e| e.to_string())?;
    std::thread::sleep(std::time::Duration::from_millis(200));
    let mut cmd = Command::new("/usr/sbin/screencapture");
    cmd.arg("-x");
    if interactive {
        cmd.arg("-i");
    }
    let status = cmd.arg(&path).status();
    let _ = window.show();
    let _ = window.set_focus();
    if !status.map_err(|e| e.to_string())?.success() || !path.exists() {
        return Err("Capture dibatalkan".into()); // e.g. Esc during region select
    }
    // Retina shots are huge; cap the longest side to keep the request small.
    let _ = Command::new("/usr/bin/sips").args(["-Z", "1600"]).arg(&path).output();
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&path);
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// Warm up the process at launch so the first message is fast too.
#[tauri::command]
fn warm(app: AppHandle, state: State<Chat>) {
    if state.stdin.lock().unwrap().is_none() {
        let _ = spawn_claude(&app, &state);
    }
}

/// Kills the process: cancels the current reply, and the next message starts a fresh session.
#[tauri::command]
fn quit(app: AppHandle, state: State<Chat>) {
    if let Some(c) = state.child.lock().unwrap().as_mut() {
        let _ = c.kill();
    }
    app.exit(0);
}

#[tauri::command]
fn stop(state: State<Chat>) {
    if let Some(c) = state.child.lock().unwrap().as_mut() {
        let _ = c.kill();
    }
}

/// Shrinks the window to a small bubble (keeping the bottom-right corner fixed) or restores it.
#[tauri::command]
fn set_collapsed(window: WebviewWindow, state: State<Chat>, collapsed: bool) -> Result<(), String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let pos = window.outer_position().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    let size = window.outer_size().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    let mut saved = state.expanded.lock().unwrap();
    let (new, min) = if collapsed {
        *saved = Some(size);
        (LogicalSize::new(56.0, 56.0), LogicalSize::new(56.0, 56.0))
    } else {
        (saved.take().unwrap_or(LogicalSize::new(380.0, 520.0)), LogicalSize::new(280.0, 300.0))
    };
    window.set_min_size(Some(min)).map_err(|e| e.to_string())?;
    window.set_size(new).map_err(|e| e.to_string())?;
    window
        .set_position(tauri::LogicalPosition::new(pos.x + size.width - new.width, pos.y + size.height - new.height))
        .map_err(|e| e.to_string())
}

fn toggle(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

/// Lets the window sit on top of other apps' fullscreen Spaces.
fn float_over_fullscreen(window: &WebviewWindow) {
    let Ok(ptr) = window.ns_window() else { return };
    // SAFETY: called on the main thread (setup); the pointer is the live NSWindow backing this window.
    let ns = unsafe {
        // Only NSPanels with the non-activating style can float above other apps' fullscreen Spaces.
        object_setClass(ptr as *mut AnyObject, FloatingPanel::class() as *const AnyClass);
        &*(ptr as *const NSWindow)
    };
    ns.setStyleMask(ns.styleMask() | NSWindowStyleMask::NonactivatingPanel);
    unsafe { &*(ptr as *const NSPanel) }.setFloatingPanel(true);
    ns.setHidesOnDeactivate(false);
    ns.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Stationary,
    );
    ns.setLevel(25); // NSStatusWindowLevel
}

fn main() {
    tauri::Builder::default()
        // A second launch just brings the existing window forward.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .manage(Chat::default())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, ev| {
                    if ev.state == ShortcutState::Pressed {
                        toggle(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![send, capture, warm, stop, quit, set_collapsed])
        .setup(|app| {
            app.set_activation_policy(ActivationPolicy::Accessory); // no Dock icon; needed to overlay fullscreen
            let win = app.get_webview_window("main").unwrap();
            float_over_fullscreen(&win);

            app.global_shortcut().register(Shortcut::new(Some(Modifiers::ALT), Code::Space))?;

            let show = MenuItem::with_id(app, "toggle", "Tampil / Sembunyi (⌥Space)", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Keluar", true, None::<&str>)?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&Menu::with_items(app, &[&show, &quit])?)
                .on_menu_event(|app, ev| match ev.id.as_ref() {
                    "toggle" => toggle(app),
                    _ => app.exit(0),
                })
                .build(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running ngambang");
}
