#![feature(let_chains)]

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;
use std::thread;
use std::thread::sleep;
use std::time::Duration;

use gumdrop::Options;
use tao::dpi::PhysicalSize;
use tao::event::Event;
use tao::event::WindowEvent;
use tao::event_loop::ControlFlow;
use tao::event_loop::EventLoop;
use tao::event_loop::EventLoopBuilder;
use tao::platform::run_return::EventLoopExtRunReturn;
use tao::window::WindowBuilder;
use wry::WebContext;
use wry::WebViewBuilder;

#[derive(Options)]
struct LandingArgs {
    /// Prints the help message.
    #[options()]
    help: bool,

    /// UUID of the storage partition for this instance. Instances of the same UUID shares stored data.
    #[options()]
    part_id: Option<String>,

    /// An alternative URL to be used as entry.
    #[options(no_short)]
    start_url: Option<String>,

    /// Window title.
    #[options(default = "LCAP")]
    title: String,

    /// The URL search param used to match OAuth code.
    #[options(no_short, default = "code")]
    code_tag: String,

    /// The URL search param used to match error descriptions.
    #[options(no_short, default = "error")]
    error_tag: String,

    /// Writes the output to file instead of stdout.
    #[options()]
    file: Option<String>,

    /// Maximum time (in milliseconds) to wait (for the page to get loaded) before showing the window.
    #[options(default = "5000")]
    wait_timeout: u64
}

struct WebViewInit {
    should_show_now: bool,
    context: Option<WebContext>
}

enum LandingEvents {
    Close(i32),
    SetVisible
}

const DEFAULT_URL: &str = "https://login.live.com/oauth20_authorize.srf?client_id=00000000402b5328&response_type=code&scope=service%3A%3Auser.auth.xboxlive.com%3A%3AMBI_SSL&redirect_uri=https%3A%2F%2Flogin.live.com%2Foauth20_desktop.srf";

fn main() -> ExitCode {
    let args = LandingArgs::parse_args_default_or_exit();

    let part_id = args
        .part_id
        .and_then(|u| uuid::Uuid::from_str(&u).ok())
        .unwrap_or(uuid::Uuid::default());

    let url = args.start_url.unwrap_or(DEFAULT_URL.to_owned());

    let code_tag = args.code_tag;
    let error_tag = args.error_tag;

    let mut events: EventLoop<LandingEvents> = EventLoopBuilder::with_user_event().build();
    let proxy = events.create_proxy();

    let WebViewInit {
        should_show_now,
        mut context
    } = prepare_webview(&part_id);

    let window = WindowBuilder::new()
        .with_title(args.title)
        .with_visible(should_show_now)
        .build(&events)
        .expect("Failed to create window");

    if let Some(ref mn) = window.current_monitor() {
        let (w, h): (u32, u32) = mn.size().into();

        fn scale(a: u32, f: f32) -> u32 { ((a as f32) * f).round() as u32 }

        window.set_inner_size(PhysicalSize::new(scale(w, 0.6), scale(h, 0.6)));
    }

    let mut wb = match context.as_mut() {
        Some(wc) => WebViewBuilder::with_web_context(wc),
        None => WebViewBuilder::new()
    };

    #[cfg(target_os = "macos")]
    {
        use wry::WebViewBuilderExtDarwin;
        wb = wb.with_data_store_identifier(part_id.as_bytes().to_owned());
    }

    if !should_show_now {
        // Set triggers (timeout, page loading) if the window is not known to be visible at creation
        thread::spawn({
            let proxy = proxy.clone();
            let timeout = args.wait_timeout;
            move || {
                sleep(Duration::from_millis(timeout));
                let _ = proxy.send_event(LandingEvents::SetVisible);
            }
        });

        let on_page_loaded = {
            let proxy = proxy.clone();

            move |_, _| {
                let _ = proxy.send_event(LandingEvents::SetVisible);
            }
        };

        wb = wb.with_on_page_load_handler(on_page_loaded);
    }

    let dump_output = {
        let fpo = args.file.to_owned();

        move |s: String| {
            if let Some(fp) = fpo.as_ref()
                && let Ok(mut f) = File::create(fp)
                && let Ok(_) = f.write_all(s.as_bytes())
            {
                return;
            }

            println!("\n{s}\n");
        }
    };

    let on_url_captured = move |s: String| {
        let Ok(u) = url::Url::from_str(&s) else {
            return true;
        };

        if let Some(ep) = u.query_pairs().find(|it| it.0 == error_tag) {
            dump_output(format!("LCAP:ERR={}", ep.1));
            let _ = proxy.send_event(LandingEvents::Close(1));
            return false; // No need to navigate further
        }

        if let Some(cp) = u.query_pairs().find(|it| it.0 == code_tag) {
            dump_output(format!("LCAP:CODE={}", cp.1));
            let _ = proxy.send_event(LandingEvents::Close(0));
            return false;
        }

        true
    };

    wb = wb.with_url(url).with_navigation_handler(on_url_captured);

    #[cfg(target_os = "linux")]
    let view_res = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;

        if let Some(vbox) = window.default_vbox() {
            wb.build_gtk(vbox)
        } else {
            wb.build_gtk(window.gtk_window())
        }
    };

    #[cfg(not(target_os = "linux"))]
    let view_res = wb.build(&window);

    // The webview instance must be hold here, or it will be destroyed
    let _view = view_res.expect("Unable to create webview instance");

    let rc = events.run_return(|ev, _, control| {
        *control = ControlFlow::Wait;

        match ev {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control = ControlFlow::ExitWithCode(1),

            Event::UserEvent(LandingEvents::Close(ec)) => *control = ControlFlow::ExitWithCode(ec),

            Event::UserEvent(LandingEvents::SetVisible) => window.set_visible(true),

            _ => {}
        }
    });

    ExitCode::from(rc as u8)
}

fn prepare_webview(uuid: &uuid::Uuid) -> WebViewInit {
    #[cfg(target_os = "macos")]
    return WebViewInit {
        should_show_now: false,
        context: None
    };

    #[cfg(not(target_os = "macos"))]
    {
        let cache_root = get_cache_root(uuid);
        let show_now = match cache_root.try_exists() {
            Ok(ex) => !ex,
            Err(_) => true
        };

        WebViewInit {
            should_show_now: show_now,
            context: Some(WebContext::new(Some(cache_root)))
        }
    }
}

fn get_cache_root(uuid: &uuid::Uuid) -> PathBuf {
    match directories::ProjectDirs::from("moe.skjsjhb", "", "LCAP") {
        Some(dirs) => dirs.data_local_dir().join(uuid.to_string()),
        None => env::home_dir()
            .unwrap_or(env::temp_dir())
            .join("LCAP")
            .join(uuid.to_string())
    }
}
