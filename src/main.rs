use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use std::thread::sleep;
use std::time::Duration;

use gumdrop::Options;
use saucers::app::App;
use saucers::options::AppOptions;
use saucers::prefs::Preferences;
use saucers::webview::events::LoadEvent;
use saucers::webview::events::NavigateEvent;
use saucers::webview::Webview;

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

const DEFAULT_URL: &str = "https://login.live.com/oauth20_authorize.srf?client_id=00000000402b5328&response_type=code&scope=service%3A%3Auser.auth.xboxlive.com%3A%3AMBI_SSL&redirect_uri=https%3A%2F%2Flogin.live.com%2Foauth20_desktop.srf";

fn main() {
    let args = LandingArgs::parse_args_default_or_exit();

    let part_id = args
        .part_id
        .and_then(|u| uuid::Uuid::from_str(&u).ok())
        .unwrap_or(uuid::Uuid::new_v4());

    let url = args.start_url.unwrap_or(DEFAULT_URL.to_owned());

    let code_tag = args.code_tag;
    let error_tag = args.error_tag;

    let cache_root = get_cache_root(&part_id);

    #[cfg(not(target_os = "macos"))]
    let show_now = !is_likely_auto_login(cache_root.as_path());

    #[cfg(target_os = "macos")]
    let show_now = false;

    let (_cc, app) = App::new(AppOptions::new("LCAP"));

    let mut prefs = Preferences::new(&app);
    prefs.set_storage_path(cache_root.to_str().unwrap());

    let webview = Arc::new(Webview::new(&prefs).unwrap());
    let size = optimal_window_size();
    webview.set_size(size.0, size.1);

    if !show_now {
        // Set triggers (timeout, page loading) if the window is not known to be visible at creation
        thread::spawn({
            let webview = Arc::downgrade(&webview);
            let timeout = args.wait_timeout;
            move || {
                sleep(Duration::from_millis(timeout));
                if let Some(webview) = webview.upgrade() {
                    webview.show();
                }
            }
        });

        webview.once::<LoadEvent>(Box::new(|w, _| w.show()));
    } else {
        webview.show();
    }

    let file_path = args.file;

    webview.set_url(url);

    webview.on::<NavigateEvent>(Box::new(move |w, nav| {
        let Ok(u) = url::Url::from_str(&nav.url()) else {
            return true;
        };

        let mut output = None;

        if let Some(ep) = u.query_pairs().find(|it| it.0 == error_tag) {
            output = Some(format!("LCAP:ERR={}", ep.1));
        }

        if let Some(cp) = u.query_pairs().find(|it| it.0 == code_tag) {
            output = Some(format!("LCAP:CODE={}", cp.1));
        }

        let Some(output) = output else {
            return true;
        };

        match file_path {
            Some(ref fp) => {
                File::create(fp)
                    .and_then(|mut f| f.write_all(output.as_bytes()))
                    .expect("Failed to write to specified file");
            }
            None => {
                println!("\n{output}\n")
            }
        };
        w.close();

        false
    }));

    app.run();
}

fn optimal_window_size() -> (i32, i32) {
    let (w, h) = screen_size::get_primary_screen_size().unwrap_or((1920u64, 1080u64));

    ((w as f32 * 0.8) as i32, (h as f32 * 0.8) as i32)
}

#[cfg(not(target_os = "macos"))]
fn is_likely_auto_login(cache: &Path) -> bool { cache.try_exists().is_ok_and(|it| it) }

fn get_cache_root(uuid: &uuid::Uuid) -> PathBuf {
    match directories::ProjectDirs::from("moe.skjsjhb", "", "LCAP") {
        Some(dirs) => dirs.data_local_dir().join(uuid.to_string()),
        None => env::home_dir()
            .unwrap_or(env::temp_dir())
            .join("LCAP")
            .join(uuid.to_string())
    }
}
