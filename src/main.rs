mod app;
mod config;
mod awww;
mod wallpaper;

use gtk4::prelude::*;
use gtk4::Application;

const APP_ID: &str = "com.wallpicker.app";

fn main() -> gtk4::glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| {
        app::build_ui(app);
    });

    app.run()
}
