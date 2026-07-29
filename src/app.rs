use glib::object::ObjectExt;
use gtk4::glib::clone;
use gtk4::prelude::*;
use gtk4::{
    gio, glib, Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry,
    GridView, HeaderBar, Image, Label, ListItem, Orientation, Overlay, PolicyType,
    ScrolledWindow, SignalListItemFactory, SingleSelection, SpinButton, Spinner,
    StringList, ToggleButton,
};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use crate::config::Config;
use crate::wallpaper::{self, WallpaperEntry, WallpaperEntryObject};
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

    let random_btn = Button::from_icon_name("media-playlist-shuffle-symbolic");
    random_btn.set_tooltip_text(Some("Wallpaper acak"));
    header.pack_end(&random_btn);

    let slideshow_btn = ToggleButton::new();
    slideshow_btn.set_icon_name("media-playlist-repeat-symbolic");
    slideshow_btn.set_tooltip_text(Some("Slide otomatis"));
    slideshow_btn.set_active(config.borrow().slideshow_enabled);
    if config.borrow().slideshow_enabled {
        slideshow_btn.add_css_class("suggested-action");
    }
    header.pack_end(&slideshow_btn);

    let settings_btn = Button::from_icon_name("emblem-system-symbolic");
    settings_btn.set_tooltip_text(Some("Pengaturan"));
    header.pack_end(&settings_btn);

    window.set_titlebar(Some(&header));

    // ---------- Main content ----------
    let root = GtkBox::new(Orientation::Vertical, 0);

    let scrolled = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vexpand(true)
        .build();

    // Status bar bawah (dibuat dulu agar bisa di-capture factory closure)
    let status_bar = GtkBox::new(Orientation::Horizontal, 8);
    status_bar.set_margin_top(6);
    status_bar.set_margin_bottom(6);
    status_bar.set_margin_start(12);
    status_bar.set_margin_end(12);

    let status_label = Rc::new(Label::new(Some("Siap.")));
    status_label.set_halign(Align::Start);
    status_label.set_hexpand(true);

    let spinner = Rc::new(Spinner::new());
    status_bar.append(status_label.as_ref());
    status_bar.append(spinner.as_ref());

    // GridView dengan widget recycling — hanya item terlihat yang dialokasikan
    let model: gio::ListStore = gio::ListStore::new::<WallpaperEntryObject>();
    let model = Rc::new(model);

    let factory = SignalListItemFactory::new();

    // setup: dipanggil sekali per ListItem (widget dibuat, lalu di-recycle)
    factory.connect_setup(|_, item| {
        let list_item = item.downcast_ref::<ListItem>().unwrap();

        let image = Image::new();
        image.set_pixel_size(180);

        let overlay = Overlay::new();
        overlay.set_child(Some(&image));

        let button = Button::builder()
            .child(&overlay)
            .css_classes(vec!["flat".to_string()])
            .build();

        let card = GtkBox::new(Orientation::Vertical, 4);
        card.set_width_request(180);
        card.append(&button);

        let label = Label::new(None);
        label.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
        label.set_max_width_chars(20);
        label.set_lines(1);
        card.append(&label);

        list_item.set_child(Some(&card));
    });

    // bind: dipanggil setiap kali item muncul/berubah di viewport
    factory.connect_bind(|_, item| {
        let list_item = item.downcast_ref::<ListItem>().unwrap();

        if let Some(entry) = list_item.item()
            .as_ref()
            .and_then(|o| o.downcast_ref::<WallpaperEntryObject>())
        {
            if let Some(child) = list_item.child() {
                if let Some(card) = child.downcast_ref::<GtkBox>() {
                    // card children: [button (overlay + image), label]
                    let thumb_path = entry.thumb_path();
                    if let Some(button) = card.first_child()
                        .and_then(|c| c.downcast::<Button>().ok())
                    {
                        if let Some(overlay) = button.child()
                            .and_then(|c| c.downcast::<Overlay>().ok())
                        {
                            if let Some(image) = overlay.child()
                                .and_then(|c| c.downcast::<Image>().ok())
                            {
                                image.set_from_file(Some(&thumb_path));
                            }
                        }
                        // Simpan path di button untuk click handler
                        let path_str = entry.full_path().to_string_lossy().to_string();
                        unsafe { button.set_data("entry-path", path_str); }
                    }
                    if let Some(second) = card.first_child()
                        .and_then(|c| c.next_sibling())
                    {
                        if let Some(label) = second.downcast_ref::<Label>() {
                            label.set_text(&entry.file_name());
                        }
                    }
                }
            }
        }
    });

    // Click handler pada setup (dipasang sekali, di-recycle bersama widget)
    factory.connect_setup(clone!(
        #[strong] status_label,
        #[strong] config,
        move |_, item| {
            let list_item = item.downcast_ref::<ListItem>().unwrap();

            if let Some(child) = list_item.child() {
                if let Some(card) = child.downcast_ref::<GtkBox>() {
                    if let Some(button) = card.first_child()
                        .and_then(|c| c.downcast::<Button>().ok())
                    {
                        button.connect_clicked(clone!(
                            #[strong] status_label,
                            #[strong] config,
                            move |btn| {
                                // Ambil path dari data button (diset di bind)
                                let path = unsafe {
                                    btn.steal_data::<String>("entry-path")
                                        .map(|s| PathBuf::from(s))
                                };

                                if let Some(path) = path {
                                    let cfg = config.borrow().clone();
                                    status_label.set_text(&format!("Menerapkan {}...", path.display()));

                                    glib::MainContext::default().spawn_local(clone!(
                                        #[strong] status_label,
                                        async move {
                                            let path_for_thread = path.clone();
                                            let result = gio::spawn_blocking(move || {
                                                awww::set_wallpaper(&path_for_thread, &cfg)
                                            }).await;

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
                            }
                        ));
                    }
                }
            }
        }
    ));

    let selection = SingleSelection::new(Some((*model).clone()));
    let grid_view = GridView::builder()
        .model(&selection)
        .factory(&factory)
        .max_columns(6)
        .build();

    scrolled.set_child(Some(&grid_view));
    root.append(&scrolled);
    root.append(&status_bar);

    window.set_child(Some(&root));

    let wallpapers: Rc<RefCell<Vec<PathBuf>>> = Rc::new(RefCell::new(Vec::new()));

    // Cek binary awww ada atau tidak, kasih tau di status bar kalau tidak ada
    if let Err(e) = awww::check_binaries_available() {
        status_label.set_text(&format!("⚠ {}", e));
    }

    // Muat grid pertama kali
    reload_grid(
        model.clone(),
        config.clone(),
        status_label.clone(),
        spinner.clone(),
        wallpapers.clone(),
        None,
    );

    // Auto-start slideshow background process jika sebelumnya aktif
    if config.borrow().slideshow_enabled {
        if !awww::is_background_slideshow_running() {
            if let Ok(pid_path) = Config::slideshow_pid_path() {
                let _ = fs::remove_file(&pid_path);
            }
            match awww::start_background_slideshow() {
                Ok(_) => status_label.set_text("✓ Slide otomatis berjalan (background)"),
                Err(e) => status_label.set_text(&format!("⚠ Gagal mulai slide: {e}")),
            }
        }
    }

    // ---------- Signal handlers ----------

    refresh_btn.connect_clicked(clone!(
        #[strong] model,
        #[strong] config,
        #[strong] status_label,
        #[strong] spinner,
        #[strong] wallpapers,
        move |_| {
            reload_grid(model.clone(), config.clone(), status_label.clone(), spinner.clone(), wallpapers.clone(), None);
        }
    ));

    search_entry.connect_changed(clone!(
        #[strong] model,
        #[strong] config,
        #[strong] status_label,
        #[strong] spinner,
        #[strong] wallpapers,
        move |entry| {
            let query = entry.text().to_string();
            reload_grid(model.clone(), config.clone(), status_label.clone(), spinner.clone(), wallpapers.clone(), Some(query));
        }
    ));

    folder_btn.connect_clicked(clone!(
        #[strong] window,
        #[strong] model,
        #[strong] config,
        #[strong] status_label,
        #[strong] spinner,
        #[strong] wallpapers,
        move |_| {
            let dialog = gtk4::FileDialog::builder()
                .title("Pilih folder wallpaper")
                .build();

            dialog.select_folder(
                Some(&window),
                gio::Cancellable::NONE,
                clone!(
                    #[strong] model,
                    #[strong] config,
                    #[strong] status_label,
                    #[strong] spinner,
                    #[strong] wallpapers,
                    move |result| {
                        if let Ok(folder) = result {
                            if let Some(path) = folder.path() {
                                config.borrow_mut().wallpaper_dir = path.to_string_lossy().to_string();
                                let _ = config.borrow().save();
                                reload_grid(model.clone(), config.clone(), status_label.clone(), spinner.clone(), wallpapers.clone(), None);
                            }
                        }
                    }
                ),
            );
        }
    ));

    slideshow_btn.connect_toggled(clone!(
        #[strong] config,
        #[strong] status_label,
        move |btn| {
            let enabled = btn.is_active();
            if enabled {
                btn.add_css_class("suggested-action");
                config.borrow_mut().slideshow_enabled = true;
                let _ = config.borrow().save();
                match awww::start_background_slideshow() {
                    Ok(_) => status_label.set_text("✓ Slide otomatis dimulai (background)"),
                    Err(e) => status_label.set_text(&format!("⚠ Gagal mulai slide: {e}")),
                }
            } else {
                btn.remove_css_class("suggested-action");
                config.borrow_mut().slideshow_enabled = false;
                let _ = config.borrow().save();
                let _ = awww::stop_background_slideshow();
                status_label.set_text("✓ Slide otomatis dihentikan");
            }
        }
    ));

    settings_btn.connect_clicked(clone!(
        #[strong] window,
        #[strong] model,
        #[strong] config,
        #[strong] status_label,
        #[strong] spinner,
        #[strong] wallpapers,
        #[strong] slideshow_btn,
        move |_| {
            open_settings_dialog(
                &window, model.clone(), config.clone(), status_label.clone(),
                spinner.clone(), wallpapers.clone(), slideshow_btn.clone(),
            );
        }
    ));

    random_btn.connect_clicked(clone!(
        #[strong] wallpapers,
        #[strong] config,
        #[strong] status_label,
        move |_| {
            let list = wallpapers.borrow();
            if list.is_empty() {
                status_label.set_text("⚠ Tidak ada wallpaper untuk dipilih acak.");
                return;
            }
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos() as usize;
            let path = list[nanos % list.len()].clone();
            drop(list);

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
                                "✓ Wallpaper acak: {}",
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

    window.present();
}

const BATCH_SIZE: usize = 50;

fn reload_grid(
    model: Rc<gio::ListStore>,
    config: Rc<RefCell<Config>>,
    status_label: Rc<Label>,
    spinner: Rc<Spinner>,
    wallpapers: Rc<RefCell<Vec<PathBuf>>>,
    query: Option<String>,
) {
    // Kosongkan model — GridView otomatis merespon perubahan
    if model.n_items() > 0 {
        let empty: &[WallpaperEntryObject] = &[];
        model.splice(0, model.n_items(), empty);
    }

    let dir = config.borrow().wallpaper_dir.clone();
    let thumb_size = config.borrow().thumb_size;

    status_label.set_text("Memindai wallpaper...");
    spinner.start();
    spinner.set_visible(true);

    glib::MainContext::default().spawn_local(clone!(
        #[strong] model,
        #[strong] status_label,
        #[strong] spinner,
        #[strong] wallpapers,
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
            *wallpapers.borrow_mut() = filtered.clone();
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
                    model.append(&WallpaperEntryObject::new(entry));
                }

                status_label.set_text(&format!("Memuat {}/{}...", loaded, total));
            }

            spinner.stop();
            spinner.set_visible(false);
            status_label.set_text(&format!("{} wallpaper dimuat.", loaded));
        }
    ));
}

/// Dialog pengaturan transisi awww + ukuran thumbnail + slide.
fn open_settings_dialog(
    parent: &ApplicationWindow,
    model: Rc<gio::ListStore>,
    config: Rc<RefCell<Config>>,
    status_label: Rc<Label>,
    spinner: Rc<Spinner>,
    wallpapers: Rc<RefCell<Vec<PathBuf>>>,
    slideshow_btn: ToggleButton,
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

    // Slideshow interval
    let slide_row = GtkBox::new(Orientation::Horizontal, 8);
    slide_row.append(&Label::new(Some("Interval slide (menit):")));
    let slide_spin = SpinButton::with_range(1.0, 120.0, 1.0);
    slide_spin.set_value(config.borrow().slideshow_interval_minutes as f64);
    slide_spin.set_hexpand(true);
    slide_row.append(&slide_spin);
    content.append(&slide_row);

    let save_btn = Button::with_label("Simpan");
    save_btn.add_css_class("suggested-action");
    content.append(&save_btn);

    dialog.set_child(Some(&content));

    save_btn.connect_clicked(clone!(
        #[strong] config,
        #[strong] dialog,
        #[strong] model,
        #[strong] status_label,
        #[strong] spinner,
        #[strong] wallpapers,
        #[strong] slideshow_btn,
        move |_| {
            let interval_changed;
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
                let new_interval = slide_spin.value() as u32;
                interval_changed = new_interval != cfg.slideshow_interval_minutes;
                cfg.slideshow_interval_minutes = new_interval;
                let _ = cfg.save();
            }
            reload_grid(model.clone(), config.clone(), status_label.clone(), spinner.clone(), wallpapers.clone(), None);

            // Restart background process jika interval berubah dan slideshow aktif
            if interval_changed && slideshow_btn.is_active() {
                let _ = awww::stop_background_slideshow();
                let _ = awww::start_background_slideshow();
            }

            dialog.close();
        }
    ));

    dialog.present();
}
