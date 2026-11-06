//! The program's own window.
//!
//! The page is shown by the operating system's web view in a native window (WebView2 on Windows,
//! WKWebView on macOS, WebKitGTK on Linux), so there is no browser to open and nothing extra to
//! ship. The page is still served by the small local server, which listens on this computer only,
//! on a port the operating system picks, so it can never clash with another program.
//!
//! Three things a browser would do for the page are done here instead:
//!
//! * **Saving files.** Web views handle downloads differently on each system, so the page hands
//!   the text of a CSV file to the program (`window.ipc.postMessage`), which shows a "Save as"
//!   dialog. (See `saveFile` in web/app.js.)
//! * **Links that lead away.** A link to another site opens in the system browser, never inside
//!   the window.
//! * **Copy and paste on a Mac**, which only work when the program has an Edit menu.
//!
//! `--check-window` is a self-test for automatic checks: it opens the window, waits until the page
//! has loaded its data, shows a statistics table and has an exchange with the program through
//! the same channel that saving uses, then exits with 0 (all well) or 1.

use crate::assets::Sources;
use crate::server;
use serde::Deserialize;
use std::path::Path;
use std::time::{Duration, Instant};
use tao::dpi::LogicalSize;
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::{Icon, WindowBuilder};
use wry::http::Request;
use wry::{NewWindowResponse, WebViewBuilder};

const TITLE: &str = "Dead Sea Scrolls Hebrew search";
const SELF_TEST_TIMEOUT: Duration = Duration::from_secs(120);
const SELF_TEST_TICK: Duration = Duration::from_millis(400);

/// What the page can ask of the program.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Message {
    Save { filename: String, text: String },
    Ping,
}

enum UserEvent {
    Save {
        filename: String,
        text: String,
    },
    Ping,
    /// the answer to one of the self-test's questions to the page
    Probe(String),
}

/// Where the self-test has got to.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Step {
    DataLoaded,
    StatisticsShown,
    ProgramAnswers,
}

fn icon() -> Option<Icon> {
    let decoder = png::Decoder::new(std::io::Cursor::new(&include_bytes!("../assets/icon-256.png")[..]));
    let mut reader = decoder.read_info().ok()?;
    let mut pixels = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut pixels).ok()?;
    pixels.truncate(frame.buffer_size());
    Icon::from_rgba(pixels, frame.width, frame.height).ok()
}

/// The "Save as" dialog, and the writing of the file.
fn save_file(filename: &str, text: &str) {
    // only the name: the page cannot choose a folder
    let name = Path::new(filename).file_name().and_then(|n| n.to_str()).filter(|n| !n.is_empty()).unwrap_or("dss-search.csv");
    let Some(mut path) = rfd::FileDialog::new().set_file_name(name).add_filter("Spreadsheet (CSV)", &["csv"]).save_file() else {
        return; // cancelled
    };
    if path.extension().is_none() {
        path.set_extension("csv");
    }
    if let Err(error) = std::fs::write(&path, text.as_bytes()) {
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("Could not save the file")
            .set_description(format!("{}: {error}", path.display()))
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }
}

/// Only web addresses are ever handed to the system: never a file path or a program name.
fn is_web_address(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

fn open_outside(url: &str) {
    if is_web_address(url) {
        let _ = open::that_detached(url);
    }
}

/// The Edit menu that makes Cmd+C, Cmd+V, Cmd+X, Cmd+A and Cmd+Z work in a web view on a Mac, with
/// the usual application and window menus.
#[cfg(target_os = "macos")]
fn install_menu() {
    use muda::{Menu, PredefinedMenuItem, Submenu};
    let build = || -> muda::Result<Menu> {
        let menu = Menu::new();
        let app = Submenu::with_items(
            TITLE,
            true,
            &[
                &PredefinedMenuItem::about(None, None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::quit(None),
            ],
        )?;
        let edit = Submenu::with_items(
            "Edit",
            true,
            &[
                &PredefinedMenuItem::undo(None),
                &PredefinedMenuItem::redo(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::cut(None),
                &PredefinedMenuItem::copy(None),
                &PredefinedMenuItem::paste(None),
                &PredefinedMenuItem::select_all(None),
            ],
        )?;
        let window = Submenu::with_items("Window", true, &[&PredefinedMenuItem::minimize(None), &PredefinedMenuItem::close_window(None)])?;
        menu.append_items(&[&app, &edit, &window])?;
        Ok(menu)
    };
    if let Ok(menu) = build() {
        menu.init_for_nsapp();
        // the menu has to stay for as long as the program runs
        std::mem::forget(menu);
    }
}

/// Windows programs started by double-click get a black console window; this program has no use for it.
#[cfg(windows)]
fn hide_console() {
    #[link(name = "kernel32")]
    extern "system" {
        fn FreeConsole() -> i32;
    }
    // SAFETY: a plain call with no arguments that only detaches this process from its console
    unsafe {
        FreeConsole();
    }
}

/// The page's own state, asked for by the self-test: the JavaScript to run, and whether its answer is the one wanted.
fn probe(step: Step) -> (&'static str, fn(&str) -> bool) {
    match step {
        Step::DataLoaded => ("(document.getElementById('status') || {}).textContent || ''", |answer| answer.contains("Ready")),
        Step::StatisticsShown => (
            "(function () { if (location.hash.indexOf('stats=freq') < 0) { location.hash = '#stats=freq&top=3'; } \
             return document.querySelectorAll('#st-table tbody tr').length; })()",
            |answer| answer == "3",
        ),
        Step::ProgramAnswers => (
            "(function () { if (!window.__sent) { window.__sent = true; window.ipc.postMessage(JSON.stringify({ type: 'ping' })); } \
             return window.__answered === true; })()",
            |answer| answer == "true",
        ),
    }
}

pub fn run(sources: Sources, self_test: bool) -> Result<(), String> {
    // Some Linux graphics drivers show a blank window with WebKitGTK's newest renderer. The page is
    // text, so the older path costs nothing.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    #[cfg(windows)]
    if !self_test {
        hide_console();
    }

    // port 0: the operating system picks a free port
    let started = server::start(sources, Some(0))?;
    let url = started.url.clone();
    let origin = url.trim_end_matches('/').to_string();

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    #[cfg(target_os = "macos")]
    install_menu();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title(TITLE)
        .with_inner_size(LogicalSize::new(1240.0, 860.0))
        .with_min_inner_size(LogicalSize::new(760.0, 520.0))
        .with_window_icon(icon())
        .build(&event_loop)
        .map_err(|e| format!("Could not open a window: {e}"))?;

    let ipc_proxy = proxy.clone();
    let allowed_origin = origin.clone();
    let builder = WebViewBuilder::new()
        .with_url(&url)
        .with_devtools(cfg!(debug_assertions))
        .with_ipc_handler(move |request: Request<String>| {
            let event = match serde_json::from_str::<Message>(request.body()) {
                Ok(Message::Save { filename, text }) => UserEvent::Save { filename, text },
                Ok(Message::Ping) => UserEvent::Ping,
                Err(_) => return,
            };
            let _ = ipc_proxy.send_event(event);
        })
        .with_navigation_handler(move |target| {
            let ours = target.starts_with(&allowed_origin)
                || target == "about:blank"
                || target.starts_with("blob:")
                || target.starts_with("data:");
            if !ours {
                open_outside(&target);
            }
            ours
        })
        .with_new_window_req_handler(|target, _| {
            open_outside(&target);
            NewWindowResponse::Deny
        });

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let webview = builder.build(&window);
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window.default_vbox().ok_or("Could not set up the window.")?;
        builder.build_gtk(vbox)
    };
    let webview = match webview {
        Ok(webview) => webview,
        Err(error) => {
            let advice = if cfg!(windows) {
                "This program shows its page with Microsoft's WebView2, which comes with Windows 11 and with current versions of Windows 10 and Microsoft Edge. If it is missing, install the \"Evergreen\" runtime from https://developer.microsoft.com/microsoft-edge/webview2/ and start the program again."
            } else {
                "The system's web view could not be started."
            };
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_title(TITLE)
                .set_description(format!("{advice}\n\n({error})"))
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
            return Err(format!("Could not start the web view: {error}"));
        }
    };

    let started_at = Instant::now();
    let mut step = Step::DataLoaded;
    let mut last_answer = String::from("(no answer yet)");
    let _server = started; // keeps the server's threads in this function's scope

    event_loop.run(move |event, _, control_flow| {
        *control_flow = if self_test { ControlFlow::WaitUntil(Instant::now() + SELF_TEST_TICK) } else { ControlFlow::Wait };
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *control_flow = ControlFlow::Exit,
            // The dialog is shown from another thread: it has to be able to use the window loop (which
            // runs here), so waiting for it here would leave it with nobody to show it.
            Event::UserEvent(UserEvent::Save { filename, text }) => {
                std::thread::spawn(move || save_file(&filename, &text));
            }
            Event::UserEvent(UserEvent::Ping) => {
                let _ = webview.evaluate_script("window.__answered = true");
            }
            Event::UserEvent(UserEvent::Probe(answer)) => {
                // the answer arrives as JSON text: a string in quotes, a number or true/false
                let answer = answer.trim().trim_matches('"').to_string();
                if probe(step).1(&answer) {
                    println!("window check: {:?} ok", step);
                    step = match step {
                        Step::DataLoaded => Step::StatisticsShown,
                        Step::StatisticsShown => Step::ProgramAnswers,
                        Step::ProgramAnswers => {
                            println!("window check: passed");
                            std::process::exit(0);
                        }
                    };
                } else {
                    last_answer = answer;
                }
            }
            Event::NewEvents(StartCause::ResumeTimeReached { .. }) if self_test => {
                if started_at.elapsed() > SELF_TEST_TIMEOUT {
                    eprintln!("window check: FAILED at {:?}; the page said: {}", step, last_answer);
                    std::process::exit(1);
                }
                let proxy = proxy.clone();
                let _ = webview.evaluate_script_with_callback(probe(step).0, move |answer| {
                    let _ = proxy.send_event(UserEvent::Probe(answer));
                });
            }
            _ => {}
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn understands_what_the_page_sends() {
        match serde_json::from_str::<Message>(r#"{"type":"save","filename":"dss-freq.csv","text":"\ufeffrank,item\n1,כל\n"}"#).unwrap() {
            Message::Save { filename, text } => {
                assert_eq!(filename, "dss-freq.csv");
                assert!(text.starts_with('\u{feff}'));
                assert!(text.contains("כל"));
            }
            _ => panic!("expected a file to save"),
        }
        assert!(matches!(serde_json::from_str::<Message>(r#"{"type":"ping"}"#).unwrap(), Message::Ping));
        assert!(serde_json::from_str::<Message>(r#"{"type":"unknown"}"#).is_err());
        assert!(serde_json::from_str::<Message>("not json").is_err());
    }

    #[test]
    fn the_window_icon_decodes() {
        assert!(icon().is_some());
    }

    #[test]
    fn only_web_addresses_are_handed_to_the_system() {
        assert!(is_web_address("https://github.com/ETCBC/dss"));
        assert!(is_web_address("http://example.org/"));
        for refused in ["file:///etc/passwd", "calc.exe", "javascript:alert(1)", "ftp://example.org/", "/usr/bin/env", ""] {
            assert!(!is_web_address(refused), "{refused}");
        }
    }
}
