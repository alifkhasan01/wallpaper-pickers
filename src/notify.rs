use notify_rust::{Notification, Urgency};

use crate::config::Config;

const APP_NAME: &str = "Wallpaper Picker";

pub fn send_notification(title: &str, body: &str, urgency: Urgency) {
    let cfg = Config::load().unwrap_or_default();
    if !cfg.notifications_enabled {
        return;
    }

    let _ = Notification::new()
        .appname(APP_NAME)
        .summary(title)
        .body(body)
        .urgency(urgency)
        .show();
}

pub fn notify_success(title: &str, body: &str) {
    send_notification(title, body, Urgency::Low);
}

pub fn notify_error(title: &str, body: &str) {
    send_notification(title, body, Urgency::Critical);
}

pub fn notify_info(title: &str, body: &str) {
    send_notification(title, body, Urgency::Normal);
}
