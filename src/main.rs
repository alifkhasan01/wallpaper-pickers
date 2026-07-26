mod app;
mod config;
mod awww;
mod wallpaper;

use gtk4::prelude::*;
use gtk4::Application;

const APP_ID: &str = "com.wallpicker.app";

fn main() -> gtk4::glib::ExitCode {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "--random" => {
                let cfg = config::Config::load().unwrap_or_default();
                let dir = cfg.wallpaper_dir.clone();
                return match wallpaper::scan_wallpapers(&dir) {
                    Ok(files) if !files.is_empty() => {
                        let nanos = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .subsec_nanos() as usize;
                        match awww::set_wallpaper(&files[nanos % files.len()], &cfg) {
                            Ok(_) => gtk4::glib::ExitCode::SUCCESS,
                            Err(e) => {
                                eprintln!("⚠ {e}");
                                gtk4::glib::ExitCode::FAILURE
                            }
                        }
                    }
                    Ok(_) => {
                        eprintln!("Tidak ada wallpaper di {dir}");
                        gtk4::glib::ExitCode::FAILURE
                    }
                    Err(e) => {
                        eprintln!("⚠ {e}");
                        gtk4::glib::ExitCode::FAILURE
                    }
                };
            }
            "--set" => {
                let path = match args.get(2) {
                    Some(p) => p,
                    None => {
                        eprintln!("Usage: wallpicker --set <path>");
                        return gtk4::glib::ExitCode::FAILURE;
                    }
                };
                let cfg = config::Config::load().unwrap_or_default();
                return match awww::set_wallpaper(std::path::Path::new(path), &cfg) {
                    Ok(_) => gtk4::glib::ExitCode::SUCCESS,
                    Err(e) => {
                        eprintln!("⚠ {e}");
                        gtk4::glib::ExitCode::FAILURE
                    }
                };
            }
            _ => {
                eprintln!("Usage: wallpicker [--random | --set <path>]");
                return gtk4::glib::ExitCode::FAILURE;
            }
        }
    }

    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| {
        app::build_ui(app);
    });

    app.run()
}
