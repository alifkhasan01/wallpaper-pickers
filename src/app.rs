use gtk4::glib::clone;
use gtk4::prelude::*;
use gtk4::{
    gio, glib, Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry, FlowBox,
    FlowBoxChild, HeaderBar, Image, Label, Orientation, Overlay, PolicyType, ScrolledWindow,
    SelectionMode, SpinButton, Spinner, StringList,
};
use std::cell::RefCell;
use std::rc::Rc;

use crate::config::Config;
use crate::wallpaper::{self, WallpaperEntry};
use crate::awww;

pub fn build_ui(app: &Application) {
    let config = Rc::new(RefCell::new(Config::load().unwrap_or_default()));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Wallpaper Picker")
        .default_width(880)
        .default_height(620)
        .build();

    // ---------- Header bar ----------
    let header = HeaderBar::new();

    let search_entry = Entry::builder()
        .placeholder_text("Cari wallpaper...")
        .hexpand(true)
        .build();
    header.set_title_widget(Some(&search_entry));

    let refresh_btn = Button::from_icon_name("view-refresh-symbolic");
    refresh_btn.set_tooltip_text(Some("Refresh"));
    header.pack_start(&refresh_btn);

    let folder_btn = Button::from_icon_name("folder-open-symbolic");
    folder_btn.set_tooltip_text(Some("Pilih folder wallpaper"));
    header.pack_start(&folder_btn);

    let settings_btn = Button::from_icon_name("emblem-system-symbolic");
    settings_btn.set_tooltip_text(Some("Pengaturan transisi"));
    header.pack_end(&settings_btn);

    window.set_titlebar(Some(&header));

    // ---------- Main content ----------
    let root = GtkBox::new(Orientation::Vertical, 0);

    let scrolled = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vexpand(true)
        .build();

    let flowbox = FlowBox::builder()
        .valign(Align::Start)
        .selection_mode(SelectionMode::None)
        .homogeneous(true)
        .row_spacing(8)
        .column_spacing(8)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    scrolled.set_child(Some(&flowbox));
    root.append(&scrolled);

    // Status bar bawah
    let status_bar = GtkBox::new(Orientation::Horizontal, 8);
    status_bar.set_margin_top(6);
    status_bar.set_margin_bottom(6);
    status_bar.set_margin_start(12);
    status_bar.set_margin_end(12);

    let status_label = Label::new(Some("Siap."));
    status_label.set_halign(Align::Start);
    status_label.set_hexpand(true);

    let spinner = Spinner::new();
    status_bar.append(&status_label);
    status_bar.append(&spinner);
    root.append(&status_bar);

    window.set_child(Some(&root));

    let flowbox = Rc::new(flowbox);
    let status_label = Rc::new(status_label);
    let spinner = Rc::new(spinner);

    // Cek binary awww ada atau tidak, kasih tau di status bar kalau tidak ada
    if let Err(e) = awww::check_binaries_available() {
        status_label.set_text(&format!("⚠ {}", e));
    }

    // Muat grid pertama kali
    reload_grid(
        flowbox.clone(),
        config.clone(),
        status_label.clone(),
        spinner.clone(),
        None,
    );

    // ---------- Signal handlers ----------

    refresh_btn.connect_clicked(clone!(
        #[strong] flowbox,
        #[strong] config,
        #[strong] status_label,
        #[strong] spinner,
        move |_| {
            reload_grid(flowbox.clone(), config.clone(), status_label.clone(), spinner.clone(), None);
        }
    ));

    search_entry.connect_changed(clone!(
        #[strong] flowbox,
        #[strong] config,
        #[strong] status_label,
        #[strong] spinner,
        move |entry| {
            let query = entry.text().to_string();
            reload_grid(flowbox.clone(), config.clone(), status_label.clone(), spinner.clone(), Some(query));
        }
    ));

    folder_btn.connect_clicked(clone!(
        #[strong] window,
        #[strong] flowbox,
        #[strong] config,
        #[strong] status_label,
        #[strong] spinner,
        move |_| {
            let dialog = gtk4::FileDialog::builder()
                .title("Pilih folder wallpaper")
                .build();

            dialog.select_folder(
                Some(&window),
                gio::Cancellable::NONE,
                clone!(
                    #[strong] flowbox,
                    #[strong] config,
                    #[strong] status_label,
                    #[strong] spinner,
                    move |result| {
                        if let Ok(folder) = result {
                            if let Some(path) = folder.path() {
                                config.borrow_mut().wallpaper_dir = path.to_string_lossy().to_string();
                                let _ = config.borrow().save();
                                reload_grid(flowbox.clone(), config.clone(), status_label.clone(), spinner.clone(), None);
                            }
                        }
                    }
                ),
            );
        }
    ));

    settings_btn.connect_clicked(clone!(
        #[strong] window,
        #[strong] config,
        #[strong] flowbox,
        #[strong] status_label,
        #[strong] spinner,
        move |_| {
            open_settings_dialog(&window, config.clone(), flowbox.clone(), status_label.clone(), spinner.clone());
        }
    ));

    window.present();
}

const BATCH_SIZE: usize = 50;

fn reload_grid(
    flowbox: Rc<FlowBox>,
    config: Rc<RefCell<Config>>,
    status_label: Rc<Label>,
    spinner: Rc<Spinner>,
    query: Option<String>,
) {
    while let Some(child) = flowbox.first_child() {
        flowbox.remove(&child);
    }

    let dir = config.borrow().wallpaper_dir.clone();
    let thumb_size = config.borrow().thumb_size;

    status_label.set_text("Memindai wallpaper...");
    spinner.start();
    spinner.set_visible(true);

    glib::MainContext::default().spawn_local(clone!(
        #[strong] flowbox,
        #[strong] status_label,
        #[strong] spinner,
        #[strong] config,
        async move {
            let scan = gio::spawn_blocking(move || wallpaper::scan_wallpapers(&dir)).await;

            let files = match scan {
                Ok(Ok(f)) => f,
                Ok(Err(e)) => {
                    spinner.stop();
                    spinner.set_visible(false);
                    status_label.set_text(&format!("⚠ {}", e));
                    return;
                }
                Err(_) => {
                    spinner.stop();
                    spinner.set_visible(false);
                    status_label.set_text("⚠ Gagal scan folder.");
                    return;
                }
            };

            let mut filtered = files;
            if let Some(q) = &query {
                let q_lower = q.to_lowercase();
                filtered.retain(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.to_lowercase().contains(&q_lower))
                        .unwrap_or(false)
                });
            }
            if filtered.is_empty() {
                spinner.stop();
                spinner.set_visible(false);
                status_label.set_text("Tidak ada wallpaper ditemukan.");
                return;
            }

            let total = filtered.len();
            let mut loaded = 0;

            for chunk in filtered.chunks(BATCH_SIZE) {
                let chunk = chunk.to_vec();
                let sz = thumb_size;

                let batch_result = gio::spawn_blocking(move || {
                    chunk
                        .into_iter()
                        .filter_map(|p| wallpaper::build_entry(&p, sz))
                        .collect::<Vec<WallpaperEntry>>()
                })
                .await;

                let Ok(entries) = batch_result else { continue };

                loaded += entries.len();

                for entry in &entries {
                    let child = build_wallpaper_card(entry, config.clone(), status_label.clone());
                    flowbox.insert(&child, -1);
                }

                status_label.set_text(&format!("Memuat {}/{}...", loaded, total));
            }

            spinner.stop();
            spinner.set_visible(false);
            status_label.set_text(&format!("{} wallpaper dimuat.", loaded));
        }
    ));
}

/// Bikin satu kartu wallpaper (thumbnail + nama file) di dalam FlowBox.
fn build_wallpaper_card(
    entry: &WallpaperEntry,
    config: Rc<RefCell<Config>>,
    status_label: Rc<Label>,
) -> FlowBoxChild {
    let card = GtkBox::new(Orientation::Vertical, 4);
    card.set_width_request(180);

    let image = Image::from_file(&entry.thumb_path);
    image.set_pixel_size(180);
    image.set_valign(Align::Start);

    let overlay = Overlay::new();
    overlay.set_child(Some(&image));

    let button = Button::builder()
        .child(&overlay)
        .css_classes(vec!["flat".to_string()])
        .build();

    let label = Label::new(Some(&entry.file_name));
    label.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
    label.set_max_width_chars(20);
    label.set_lines(1);

    card.append(&button);
    card.append(&label);

    let path = entry.full_path.clone();
    button.connect_clicked(clone!(
        #[strong] status_label,
        #[strong] config,
        move |_| {
            let path = path.clone();
            let cfg = config.borrow().clone();
            status_label.set_text(&format!("Menerapkan {}...", path.display()));

            glib::MainContext::default().spawn_local(clone!(
                #[strong] status_label,
                async move {
                    let path_for_thread = path.clone();
                    let result = gio::spawn_blocking(move || {
                        awww::set_wallpaper(&path_for_thread, &cfg)
                    })
                    .await;

                    match result {
                        Ok(Ok(())) => {
                            status_label.set_text(&format!(
                                "✓ Wallpaper diset: {}",
                                path.file_name().unwrap_or_default().to_string_lossy()
                            ));
                        }
                        Ok(Err(e)) => {
                            status_label.set_text(&format!("⚠ Gagal set wallpaper: {}", e));
                        }
                        Err(_) => {
                            status_label.set_text("⚠ Gagal set wallpaper (thread error).");
                        }
                    }
                }
            ));
        }
    ));

    let flow_child = FlowBoxChild::new();
    flow_child.set_child(Some(&card));
    flow_child
}

/// Dialog pengaturan transisi awww + ukuran thumbnail.
fn open_settings_dialog(
    parent: &ApplicationWindow,
    config: Rc<RefCell<Config>>,
    flowbox: Rc<FlowBox>,
    status_label: Rc<Label>,
    spinner: Rc<Spinner>,
) {
    let dialog = gtk4::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Pengaturan")
        .default_width(360)
        .build();

    let content = GtkBox::new(Orientation::Vertical, 12);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);

    // Transition type
    let types = ["simple", "fade", "wipe", "wave", "grow", "center", "outer", "random"];
    let type_list = StringList::new(&types);
    let current_idx = types
        .iter()
        .position(|t| *t == config.borrow().transition_type)
        .unwrap_or(2) as u32;

    let type_row = GtkBox::new(Orientation::Horizontal, 8);
    type_row.append(&Label::new(Some("Tipe transisi:")));
    let type_dropdown = gtk4::DropDown::new(Some(type_list), gtk4::Expression::NONE);
    type_dropdown.set_selected(current_idx);
    type_dropdown.set_hexpand(true);
    type_row.append(&type_dropdown);
    content.append(&type_row);

    // Duration
    let dur_row = GtkBox::new(Orientation::Horizontal, 8);
    dur_row.append(&Label::new(Some("Durasi (detik):")));
    let dur_spin = SpinButton::with_range(0.1, 5.0, 0.1);
    dur_spin.set_value(config.borrow().transition_duration as f64);
    dur_spin.set_hexpand(true);
    dur_row.append(&dur_spin);
    content.append(&dur_row);

    // FPS
    let fps_row = GtkBox::new(Orientation::Horizontal, 8);
    fps_row.append(&Label::new(Some("FPS transisi:")));
    let fps_spin = SpinButton::with_range(24.0, 144.0, 1.0);
    fps_spin.set_value(config.borrow().transition_fps as f64);
    fps_spin.set_hexpand(true);
    fps_row.append(&fps_spin);
    content.append(&fps_row);

    // Columns / thumb size info
    let thumb_row = GtkBox::new(Orientation::Horizontal, 8);
    thumb_row.append(&Label::new(Some("Ukuran thumbnail (px):")));
    let thumb_spin = SpinButton::with_range(100.0, 400.0, 20.0);
    thumb_spin.set_value(config.borrow().thumb_size as f64);
    thumb_spin.set_hexpand(true);
    thumb_row.append(&thumb_spin);
    content.append(&thumb_row);

    let save_btn = Button::with_label("Simpan");
    save_btn.add_css_class("suggested-action");
    content.append(&save_btn);

    dialog.set_child(Some(&content));

    save_btn.connect_clicked(clone!(
        #[strong] config,
        #[strong] dialog,
        #[strong] flowbox,
        #[strong] status_label,
        #[strong] spinner,
        move |_| {
            {
                let mut cfg = config.borrow_mut();
                let selected = type_dropdown.selected();
                cfg.transition_type = types
                    .get(selected as usize)
                    .unwrap_or(&"wipe")
                    .to_string();
                cfg.transition_duration = dur_spin.value() as f32;
                cfg.transition_fps = fps_spin.value() as u32;
                cfg.thumb_size = thumb_spin.value() as u32;
                let _ = cfg.save();
            }
            reload_grid(flowbox.clone(), config.clone(), status_label.clone(), spinner.clone(), None);
            dialog.close();
        }
    ));

    dialog.present();
}
