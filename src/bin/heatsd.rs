use std::sync::Mutex;
use std::{process, thread, time::Duration};

use global_hotkey::GlobalHotKeyManager;

use heats::app::{self, State};
use heats::config::{self, Config};
use heats::hotkey;
use heats::ipc;

static BOOT_PARAMS: Mutex<Option<(Config, GlobalHotKeyManager, u32)>> = Mutex::new(None);

fn boot() -> (State, iced::Task<app::Message>) {
    let params = BOOT_PARAMS
        .lock()
        .unwrap()
        .take()
        .expect("boot() called more than once");
    State::new(params.0, params.1, params.2)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let subcmd = args.get(1).map(|s| s.as_str());

    match subcmd {
        Some("stop") => cmd_stop(),
        Some("restart") => cmd_restart(),
        Some("service") => {
            let action = args.get(2).map(|s| s.as_str());
            match action {
                Some("install") => cmd_service_install(),
                Some("uninstall") => cmd_service_uninstall(),
                _ => {
                    eprintln!("Usage: heatsd service <install|uninstall>");
                    process::exit(2);
                }
            }
        }
        Some(other) => {
            eprintln!("Unknown command: {other}");
            eprintln!("Usage: heatsd [stop|restart|service <install|uninstall>]");
            process::exit(2);
        }
        None => cmd_run(),
    }
}

// ---- Run (default) ----

fn cmd_run() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let config = config::load();

    // Clean up stale socket from previous run
    let sock = ipc::socket_path();
    if sock.exists() {
        let _ = std::fs::remove_file(&sock);
        tracing::info!("Removed stale socket: {}", sock.display());
    }

    // Write PID file
    ipc::write_pid();

    // Set up signal handler for graceful shutdown
    let _ = ctrlc::set_handler(move || {
        ipc::remove_pid();
        let sock = ipc::socket_path();
        let _ = std::fs::remove_file(&sock);
        process::exit(0);
    });

    // Initialize global hotkey manager on the main thread (macOS requirement)
    let (manager, registered_hotkey) = hotkey::init_manager(&config.hotkey);
    let hotkey_id = registered_hotkey.id();

    tracing::info!("Starting Heats launcher");

    *BOOT_PARAMS.lock().unwrap() = Some((config, manager, hotkey_id));

    let result = iced::daemon(boot, State::update, State::view)
        .title(State::title)
        .subscription(State::subscription)
        .theme(State::theme)
        .style(State::style)
        .run();

    // Cleanup on normal exit
    ipc::remove_pid();
    let _ = std::fs::remove_file(ipc::socket_path());

    if let Err(e) = result {
        eprintln!("heatsd: {e}");
        process::exit(1);
    }
}

// ---- Stop ----

fn cmd_stop() {
    match ipc::read_pid() {
        Some(pid) => {
            eprintln!("Stopping heatsd (pid {pid})...");
            let ret = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
            if ret != 0 {
                eprintln!("Failed to send SIGTERM to pid {pid}");
                process::exit(1);
            }
            // Wait for process to exit
            for _ in 0..20 {
                thread::sleep(Duration::from_millis(100));
                let alive = unsafe { libc::kill(pid as i32, 0) };
                if alive != 0 {
                    eprintln!("heatsd stopped.");
                    ipc::remove_pid();
                    return;
                }
            }
            eprintln!("heatsd did not stop within 2 seconds.");
            process::exit(1);
        }
        None => {
            eprintln!("heatsd is not running (no PID file found).");
            process::exit(1);
        }
    }
}

// ---- Restart ----

fn cmd_restart() {
    // Stop if running (ignore errors if not running)
    if ipc::read_pid().is_some() {
        cmd_stop();
    }

    thread::sleep(Duration::from_millis(300));

    let exe = std::env::current_exe().expect("Failed to get current executable path");
    eprintln!("Starting heatsd...");

    let child = std::process::Command::new(&exe)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    match child {
        Ok(_) => eprintln!("heatsd started."),
        Err(e) => {
            eprintln!("Failed to start heatsd: {e}");
            process::exit(1);
        }
    }
}

// ---- Service (launchd) ----

const PLIST_LABEL: &str = "com.heats.daemon";

fn plist_path() -> std::path::PathBuf {
    dirs::home_dir()
        .expect("Could not determine home directory")
        .join("Library/LaunchAgents")
        .join(format!("{PLIST_LABEL}.plist"))
}

fn cmd_service_install() {
    let exe = std::env::current_exe()
        .expect("Failed to get current executable path")
        .to_string_lossy()
        .to_string();

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{PLIST_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
    </array>
    <key>KeepAlive</key>
    <true/>
    <key>RunAtLoad</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/heatsd.out.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/heatsd.err.log</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>heats=info</string>
    </dict>
</dict>
</plist>"#
    );

    let path = plist_path();

    // Ensure LaunchAgents directory exists
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Err(e) = std::fs::write(&path, &plist_content) {
        eprintln!("Failed to write plist: {e}");
        process::exit(1);
    }
    eprintln!("Wrote {}", path.display());

    let status = std::process::Command::new("launchctl")
        .args(["load", &path.to_string_lossy()])
        .status();

    match status {
        Ok(s) if s.success() => eprintln!("Service installed and loaded."),
        Ok(s) => {
            eprintln!("launchctl load exited with {s}");
            process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to run launchctl: {e}");
            process::exit(1);
        }
    }
}

fn cmd_service_uninstall() {
    let path = plist_path();

    if !path.exists() {
        eprintln!("Service is not installed.");
        process::exit(1);
    }

    let status = std::process::Command::new("launchctl")
        .args(["unload", &path.to_string_lossy()])
        .status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => eprintln!("launchctl unload exited with {s}"),
        Err(e) => eprintln!("Failed to run launchctl: {e}"),
    }

    if let Err(e) = std::fs::remove_file(&path) {
        eprintln!("Failed to remove plist: {e}");
        process::exit(1);
    }

    eprintln!("Service uninstalled.");
}
