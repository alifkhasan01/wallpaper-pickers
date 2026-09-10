mod app;
mod config;
mod awww;
mod notify;
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
                            Ok(_) => {
                                notify::notify_success("Wallpaper Acak", "Wallpaper acak berhasil diset");
                                gtk4::glib::ExitCode::SUCCESS
                            }
                            Err(e) => {
                                eprintln!("⚠ {e}");
                                notify::notify_error("Gagal Set Wallpaper", &e.to_string());
                                gtk4::glib::ExitCode::FAILURE
                            }
                        }
                    }
                    Ok(_) => {
                        eprintln!("Tidak ada wallpaper di {dir}");
                        notify::notify_error("Tidak Ada Wallpaper", &format!("Tidak ada wallpaper di {}", dir));
                        gtk4::glib::ExitCode::FAILURE
                    }
                    Err(e) => {
                        eprintln!("⚠ {e}");
                        notify::notify_error("Gagal Scan", &e.to_string());
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
                    Ok(_) => {
                        notify::notify_success("Wallpaper Diset", &format!("Berhasil: {}", path));
                        gtk4::glib::ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("⚠ {e}");
                        notify::notify_error("Gagal Set Wallpaper", &e.to_string());
                        gtk4::glib::ExitCode::FAILURE
                    }
                };
            }
            "--current" => {
                return match awww::get_current_wallpaper() {
                    Some(p) => {
                        println!("{}", p.display());
                        gtk4::glib::ExitCode::SUCCESS
                    }
                    None => {
                        eprintln!("Tidak ada wallpaper aktif.");
                        gtk4::glib::ExitCode::FAILURE
                    }
                };
            }
            "--slideshow-bg" => {
                let cfg = match config::Config::load() {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("⚠ Gagal load config: {e}");
                        return gtk4::glib::ExitCode::FAILURE;
                    }
                };
                if !cfg.slideshow_enabled {
                    return gtk4::glib::ExitCode::SUCCESS;
                }
                run_slideshow_background(&cfg);
                return gtk4::glib::ExitCode::SUCCESS;
            }
            _ => {
                eprintln!("Usage: wallpicker [--random | --set <path> | --current | --slideshow-bg]");
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

fn run_slideshow_background(_: &config::Config) {
    loop {
        let cfg = match config::Config::load() {
            Ok(c) => c,
            Err(_) => break,
        };
        if !cfg.slideshow_enabled {
            break;
        }
        let interval_secs = cfg.slideshow_interval_minutes.max(1) as u64 * 60;

        let files = match wallpaper::scan_wallpapers(&cfg.wallpaper_dir) {
            Ok(f) if !f.is_empty() => f,
            _ => {
                std::thread::sleep(std::time::Duration::from_secs(interval_secs));
                continue;
            }
        };

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos() as usize;
        let path = &files[nanos % files.len()];
        if let Err(e) = awww::set_wallpaper(path, &cfg) {
            notify::notify_error("Slide Gagal", &e.to_string());
        } else {
            let fname = path.file_name().unwrap_or_default().to_string_lossy();
            notify::notify_info("Slide Otomatis", &format!("Wallpaper: {}", fname));
        }

        std::thread::sleep(std::time::Duration::from_secs(interval_secs));
    }
}
