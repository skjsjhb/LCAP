#![feature(let_chains)]

use clap::Parser;
use std::env;
use std::fs::File;
use std::io::Write;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use tao::dpi::PhysicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::platform::run_return::EventLoopExtRunReturn;
use tao::window::WindowBuilder;
use wry::{WebContext, WebViewBuilder};

const FALLBACK_ENT_URL: &str = "https://login.live.com/oauth20_authorize.srf?client_id=00000000402b5328&response_type=code&scope=service%3A%3Auser.auth.xboxlive.com%3A%3AMBI_SSL&redirect_uri=https%3A%2F%2Flogin.live.com%2Foauth20_desktop.srf";

#[derive(Parser)]
struct LandingArgs {
    /// UUID of the storage partition for this instance.
    /// A random one is generated if unspecified or invalid.
    ///
    /// Browser data are shared between instances of the same UUID.
    /// Making it useful for keeping user logged in.
    #[arg(short, long, value_name = "UUID")]
    part_id: Option<String>,

    /// An alternative URL to be used as entry.
    #[arg(short, long)]
    start_url: Option<String>,

    /// Window title.
    #[arg(short, long)]
    title: Option<String>,

    /// The URL search param used to match OAuth code.
    #[arg(short, long)]
    code_tag: Option<String>,

    /// The URL search param used to match error descriptions.
    #[arg(short, long)]
    error_tag: Option<String>,

    /// Writes the output to file instead of stdout.
    #[arg(short, long)]
    file: Option<String>,
}

fn main() {
    let args = LandingArgs::parse();
    let mut a = env::args();
    a.next();

    let part_id = args
        .part_id
        .and_then(|u| uuid::Uuid::from_str(&u).ok())
        .unwrap_or(uuid::Uuid::new_v4());

    let url = args.start_url.unwrap_or(FALLBACK_ENT_URL.to_owned());

    let code_tag = args.code_tag.unwrap_or("code".to_owned());
    let error_tag = args.error_tag.unwrap_or("error".to_owned());

    let mut events = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(args.title.unwrap_or("LCAP".to_owned()))
        .build(&events)
        .expect("Failed to create window");

    if let Some(ref mn) = window.current_monitor() {
        let (w, h): (u32, u32) = mn.size().into();

        fn scale(a: u32, f: f32) -> u32 {
            ((a as f32) * f).round() as u32
        }

        window.set_inner_size(PhysicalSize::new(scale(w, 0.6), scale(h, 0.6)));
    }

    let mut ctx = create_context(&part_id);
    let mut wb = WebViewBuilder::with_web_context(&mut ctx);

    #[cfg(target_os = "macos")]
    {
        use wry::WebViewBuilderExtDarwin;
        wb = wb.with_data_store_identifier(part_id.as_bytes().to_owned());
    }

    let running = Arc::new(AtomicBool::new(true));
    let exit_code = Arc::new(AtomicI32::new(1));

    let set_exit = {
        let r = running.clone();
        let ec = exit_code.clone();

        move |code: i32| {
            r.store(false, Ordering::SeqCst);
            ec.store(code, Ordering::SeqCst);
        }
    };

    let dump_output = {
        let fpo = args.file.to_owned();

        move |s: String| {
            if let Some(fp) = fpo.as_ref()
                && let Ok(mut f) = File::create(fp)
                && let Ok(_) = f.write_all(s.as_bytes())
            {
                return;
            }

            println!("{s}");
        }
    };

    let on_url_captured = move |s: String| {
        let Ok(u) = url::Url::from_str(&s) else {
            return true;
        };

        if let Some(ep) = u.query_pairs().find(|it| it.0 == error_tag) {
            dump_output(format!("LCAP:ERR={}", ep.1));
            set_exit(1);
            return false; // No need to navigate further
        }

        if let Some(cp) = u.query_pairs().find(|it| it.0 == code_tag) {
            dump_output(format!("LCAP:CODE={}", cp.1));
            set_exit(0);
            return false;
        }

        true
    };

    wb = wb.with_url(url).with_navigation_handler(on_url_captured);

    #[cfg(target_os = "linux")]
    use tao::platform::unix::WindowExtUnix;
    #[cfg(target_os = "linux")]
    use wry::WebViewBuilderExtUnix;
    #[cfg(target_os = "linux")]
    let view_res = wb.build_gtk(window.gtk_window());

    #[cfg(not(target_os = "linux"))]
    let view_res = wb.build(&window);

    // The webview instance must be hold here, or it will be destroyed
    let _view = view_res.expect("Unable to create webview instance");

    events.run_return(|ev, _, control| {
        *control = ControlFlow::Wait;

        if !running.load(Ordering::SeqCst) {
            *control = ControlFlow::ExitWithCode(exit_code.load(Ordering::SeqCst))
        }

        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = ev
        {
            *control = ControlFlow::ExitWithCode(1)
        }
    });
}

fn create_context(uuid: &uuid::Uuid) -> WebContext {
    let cache_root = match directories::ProjectDirs::from("moe.skjsjhb", "", "LCAP") {
        Some(dirs) => dirs.data_local_dir().join(uuid.to_string()),
        None => env::home_dir()
            .unwrap_or(env::temp_dir())
            .join("LCAP")
            .join(uuid.to_string()),
    };

    WebContext::new(Some(cache_root))
}
