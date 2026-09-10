use glib::object::ObjectExt;
use gtk4::gdk;
use gtk4::glib::clone;
use gtk4::prelude::*;
use gtk4::{
    gio, glib, Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry,
    EventControllerKey, GridView, HeaderBar, Image, Label, ListItem, Orientation, Overlay,
    PolicyType, ScrolledWindow, SignalListItemFactory, SingleSelection, SpinButton, Spinner,
    StringList, Switch, ToggleButton,
};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use crate::config::Config;
use crate::wallpaper::{self, WallpaperEntry, WallpaperEntryObject};
use crate::awww;
use crate::notify;

/// State bersama yang dipakai di seluruh handler UI.
struct GridCtx {
    model: Rc<gio::ListStore>,
    config: Rc<RefCell<Config>>,
    status_label: Rc<Label>,
    spinner: Rc<Spinner>,
    wallpapers: Rc<RefCell<Vec<PathBuf>>>,
    current_wp: Rc<RefCell<Option<PathBuf>>>,
}

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

    // CSS untuk highlight wallpaper yang sedang aktif
    let css_provider = gtk4::CssProvider::new();
    css_provider.load_from_data(
        ".wp-current { outline: 3px solid @theme_selected_bg_color; outline-offset: -3px; border-radius: 12px; }",
    );
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css_provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    // ---------- Main content ----------
    let root = GtkBox::new(Orientation::Vertical, 0);

    let scrolled = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vexpand(true)
        .build();

    // Status bar bawah
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

    let ctx = Rc::new(GridCtx {
        model: Rc::new(model),
        config: config.clone(),
        status_label: status_label.clone(),
        spinner: spinner.clone(),
        wallpapers: Rc::new(RefCell::new(Vec::new())),
        // Wallpaper yang sedang aktif (dibaca dari awww / cache)
        current_wp: Rc::new(RefCell::new(None)),
    });

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
    factory.connect_bind(clone!(
        #[strong] ctx,
        move |_, item| {
            let list_item = item.downcast_ref::<ListItem>().unwrap();

            if let Some(entry) = list_item.item()
                .as_ref()
                .and_then(|o| o.downcast_ref::<WallpaperEntryObject>())
            {
                let is_current = ctx
                    .current_wp
                    .borrow()
                    .as_ref()
                    .map(|c| wallpaper::same_path(c, &entry.full_path()))
                    .unwrap_or(false);

                if let Some(child) = list_item.child() {
                    if let Some(card) = child.downcast_ref::<GtkBox>() {
                        // card children: [button (overlay + image), label]
                        let thumb_path = entry.thumb_path();
                        if let Some(button) = card.first_child()
                            .and_then(|c| c.downcast::<Button>().ok())
                        {
                            if is_current {
                                button.add_css_class("wp-current");
                            } else {
                                button.remove_css_class("wp-current");
                            }
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
        }
    ));

    // Click handler pada setup (dipasang sekali, di-recycle bersama widget)
    factory.connect_setup(clone!(
        #[strong] ctx,
        move |_, item| {
            let list_item = item.downcast_ref::<ListItem>().unwrap();

            if let Some(child) = list_item.child() {
                if let Some(card) = child.downcast_ref::<GtkBox>() {
                    if let Some(button) = card.first_child()
                        .and_then(|c| c.downcast::<Button>().ok())
                    {
                        button.connect_clicked(clone!(
                            #[strong] ctx,
                            move |btn| {
                                // Ambil path dari data button (diset di bind)
                                let path = unsafe {
                                    btn.steal_data::<String>("entry-path")
                                        .map(PathBuf::from)
                                };

                                if let Some(path) = path {
                                    spawn_apply_wallpaper(path, ctx.clone());
                                }
                            }
                        ));
                    }
                }
            }
        }
    ));

    let selection = SingleSelection::new(Some((*ctx.model).clone()));
    let selection = Rc::new(selection);
    let grid_view = GridView::builder()
        .model(selection.as_ref())
        .factory(&factory)
        .max_columns(6)
        .build();

    // Grid bisa menerima fokus untuk navigasi keyboard
    grid_view.set_focusable(true);

    // Enter = terapkan wallpaper terpilih, Esc = bersihkan pencarian & balik ke grid.
    // Panah kiri/kanan/atas/bawah ditangani native oleh GridView (pindah seleksi).
    let key_ctrl = EventControllerKey::new();
    key_ctrl.connect_key_pressed(clone!(
        #[strong] ctx,
        #[strong] selection,
        #[strong] search_entry,
        #[strong] grid_view,
        move |_, keyval, _, _| {
            match keyval {
                gdk::Key::Return | gdk::Key::KP_Enter => {
                    let pos = selection.selected();
                    if let Some(obj) = ctx.model.item(pos) {
                        if let Some(entry_obj) = obj.downcast_ref::<WallpaperEntryObject>() {
                            spawn_apply_wallpaper(entry_obj.full_path(), ctx.clone());
                        }
                    }
                    glib::Propagation::Stop
                }
                gdk::Key::Escape => {
                    if !search_entry.text().is_empty() {
                        search_entry.set_text("");
                    }
                    grid_view.grab_focus();
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        }
    ));
    grid_view.add_controller(key_ctrl);

    scrolled.set_child(Some(&grid_view));
    root.append(&scrolled);
    root.append(&status_bar);

    window.set_child(Some(&root));

    // Cek binary awww ada atau tidak, kasih tau di status bar kalau tidak ada
    if let Err(e) = awww::check_binaries_available() {
        status_label.set_text(&format!("⚠ {}", e));
        notify::notify_error("Binary Tidak Ditemukan", &e.to_string());
    }

    // Muat grid pertama kali
    reload_grid(ctx.clone(), true, None);

    // Enter di kolom pencarian → pindah fokus ke grid
    search_entry.connect_activate(clone!(
        #[strong] grid_view,
        move |_| {
            grid_view.grab_focus();
        }
    ));

    // Auto-start slideshow background process jika sebelumnya aktif
    if config.borrow().slideshow_enabled && !awww::is_background_slideshow_running() {
        if let Ok(pid_path) = Config::slideshow_pid_path() {
            let _ = fs::remove_file(&pid_path);
        }
        match awww::start_background_slideshow() {
            Ok(_) => {
                status_label.set_text("✓ Slide otomatis berjalan (background)");
                notify::notify_info("Slide Otomatis", "Slide otomatis berjalan di background");
            }
            Err(e) => {
                status_label.set_text(&format!("⚠ Gagal mulai slide: {e}"));
                notify::notify_error("Gagal Mulai Slide", &e.to_string());
            }
        }
    }

    // ---------- Signal handlers ----------

    refresh_btn.connect_clicked(clone!(
        #[strong] ctx,
        move |_| {
            reload_grid(ctx.clone(), true, None);
        }
    ));

    search_entry.connect_changed(clone!(
        #[strong] ctx,
        move |entry| {
            let query = entry.text().to_string();
            reload_grid(ctx.clone(), false, Some(query));
        }
    ));

    folder_btn.connect_clicked(clone!(
        #[strong] window,
        #[strong] ctx,
        move |_| {
            let dialog = gtk4::FileDialog::builder()
                .title("Pilih folder wallpaper")
                .build();

            dialog.select_folder(
                Some(&window),
                gio::Cancellable::NONE,
                clone!(
                    #[strong] ctx,
                    move |result| {
                        if let Ok(folder) = result {
                            if let Some(path) = folder.path() {
                                ctx.config.borrow_mut().wallpaper_dir =
                                    path.to_string_lossy().to_string();
                                let _ = ctx.config.borrow().save();
                                reload_grid(ctx.clone(), true, None);
                            }
                        }
                    }
                ),
            );
        }
    ));

    slideshow_btn.connect_toggled(clone!(
        #[strong(rename_to = config)] ctx.config,
        #[strong(rename_to = status_label)] ctx.status_label,
        move |btn| {
            let enabled = btn.is_active();
            if enabled {
                btn.add_css_class("suggested-action");
                config.borrow_mut().slideshow_enabled = true;
                let _ = config.borrow().save();
                match awww::start_background_slideshow() {
                    Ok(_) => {
                        status_label.set_text("✓ Slide otomatis dimulai (background)");
                        notify::notify_info("Slide Dimulai", "Slide otomatis dimulai di background");
                    }
                    Err(e) => {
                        status_label.set_text(&format!("⚠ Gagal mulai slide: {e}"));
                        notify::notify_error("Gagal Mulai Slide", &e.to_string());
                    }
                }
            } else {
                btn.remove_css_class("suggested-action");
                config.borrow_mut().slideshow_enabled = false;
                let _ = config.borrow().save();
                let _ = awww::stop_background_slideshow();
                status_label.set_text("✓ Slide otomatis dihentikan");
                notify::notify_info("Slide Dihentikan", "Slide otomatis telah dihentikan");
            }
        }
    ));

    settings_btn.connect_clicked(clone!(
        #[strong] window,
        #[strong] ctx,
        #[strong] slideshow_btn,
        move |_| {
            open_settings_dialog(&window, ctx.clone(), slideshow_btn.clone());
        }
    ));

    random_btn.connect_clicked(clone!(
        #[strong] ctx,
        move |_| {
            let path = {
                let list = ctx.wallpapers.borrow();
                if list.is_empty() {
                    None
                } else {
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .subsec_nanos() as usize;
                    Some(list[nanos % list.len()].clone())
                }
            };

            match path {
                Some(p) => spawn_apply_wallpaper(p, ctx.clone()),
                None => ctx.status_label.set_text("⚠ Tidak ada wallpaper untuk dipilih acak."),
            }
        }
    ));

    gtk4::prelude::GtkWindowExt::set_focus(&window, Some(&grid_view));
    window.present();
}

const BATCH_SIZE: usize = 50;

/// Terapkan wallpaper secara async, lalu update indikator "aktif"
/// (highlight kartu di grid + status bar).
fn spawn_apply_wallpaper(path: PathBuf, ctx: Rc<GridCtx>) {
    let cfg = ctx.config.borrow().clone();
    ctx.status_label
        .set_text(&format!("Menerapkan {}...", path.display()));

    glib::MainContext::default().spawn_local(clone!(
        #[strong] path,
        #[strong] ctx,
        async move {
            let path_for_thread = path.clone();
            let result = gio::spawn_blocking(move || {
                awww::set_wallpaper(&path_for_thread, &cfg)
            })
            .await;

            match result {
                Ok(Ok(())) => {
                    *ctx.current_wp.borrow_mut() = Some(path.clone());
                    // Paksa rebind item terlihat agar highlight ikut pindah
                    ctx.model.items_changed(0, ctx.model.n_items(), ctx.model.n_items());
                    let fname = path.file_name().unwrap_or_default().to_string_lossy();
                    ctx.status_label.set_text(&format!("✓ Wallpaper diset: {}", fname));
                    notify::notify_success("Wallpaper Diset", &format!("Berhasil: {}", fname));
                }
                Ok(Err(e)) => {
                    ctx.status_label
                        .set_text(&format!("⚠ Gagal set wallpaper: {}", e));
                    notify::notify_error("Gagal Set Wallpaper", &e.to_string());
                }
                Err(_) => {
                    ctx.status_label
                        .set_text("⚠ Gagal set wallpaper (thread error).");
                    notify::notify_error("Gagal Set Wallpaper", "Thread error");
                }
            }
        }
    ));
}

fn reload_grid(ctx: Rc<GridCtx>, refresh_current: bool, query: Option<String>) {
    // Kosongkan model — GridView otomatis merespon perubahan
    if ctx.model.n_items() > 0 {
        let empty: &[WallpaperEntryObject] = &[];
        ctx.model.splice(0, ctx.model.n_items(), empty);
    }

    let dir = ctx.config.borrow().wallpaper_dir.clone();
    let thumb_size = ctx.config.borrow().thumb_size;

    ctx.status_label.set_text("Memindai wallpaper...");
    ctx.spinner.start();
    ctx.spinner.set_visible(true);

    glib::MainContext::default().spawn_local(clone!(
        #[strong] ctx,
        async move {
            let scan = gio::spawn_blocking(move || wallpaper::scan_wallpapers(&dir)).await;

            let files = match scan {
                Ok(Ok(f)) => f,
                Ok(Err(e)) => {
                    ctx.spinner.stop();
                    ctx.spinner.set_visible(false);
                    ctx.status_label.set_text(&format!("⚠ {}", e));
                    return;
                }
                Err(_) => {
                    ctx.spinner.stop();
                    ctx.spinner.set_visible(false);
                    ctx.status_label.set_text("⚠ Gagal scan folder.");
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
            *ctx.wallpapers.borrow_mut() = filtered.clone();
            if filtered.is_empty() {
                ctx.spinner.stop();
                ctx.spinner.set_visible(false);
                ctx.status_label.set_text("Tidak ada wallpaper ditemukan.");
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
                    ctx.model.append(&WallpaperEntryObject::new(entry));
                }

                ctx.status_label.set_text(&format!("Memuat {}/{}...", loaded, total));
            }

            ctx.spinner.stop();
            ctx.spinner.set_visible(false);

            // Deteksi wallpaper yang sedang aktif (baca dari awww/cache)
            if refresh_current {
                let cur = gio::spawn_blocking(awww::get_current_wallpaper)
                    .await
                    .unwrap_or(None);
                *ctx.current_wp.borrow_mut() = cur;
                ctx.model.items_changed(0, ctx.model.n_items(), ctx.model.n_items());
            }

            let active_txt = match ctx.current_wp.borrow().as_ref() {
                Some(c) => format!(
                    " — aktif: {}",
                    c.file_name().unwrap_or_default().to_string_lossy()
                ),
                None => String::new(),
            };
            ctx.status_label
                .set_text(&format!("{} wallpaper dimuat.{}", loaded, active_txt));
        }
    ));
}

/// Dialog pengaturan transisi awww + ukuran thumbnail + slide.
fn open_settings_dialog(parent: &ApplicationWindow, ctx: Rc<GridCtx>, slideshow_btn: ToggleButton) {
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
        .position(|t| *t == ctx.config.borrow().transition_type)
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
    dur_spin.set_value(ctx.config.borrow().transition_duration as f64);
    dur_spin.set_hexpand(true);
    dur_row.append(&dur_spin);
    content.append(&dur_row);

    // FPS
    let fps_row = GtkBox::new(Orientation::Horizontal, 8);
    fps_row.append(&Label::new(Some("FPS transisi:")));
    let fps_spin = SpinButton::with_range(24.0, 144.0, 1.0);
    fps_spin.set_value(ctx.config.borrow().transition_fps as f64);
    fps_spin.set_hexpand(true);
    fps_row.append(&fps_spin);
    content.append(&fps_row);

    // Columns / thumb size info
    let thumb_row = GtkBox::new(Orientation::Horizontal, 8);
    thumb_row.append(&Label::new(Some("Ukuran thumbnail (px):")));
    let thumb_spin = SpinButton::with_range(100.0, 400.0, 20.0);
    thumb_spin.set_value(ctx.config.borrow().thumb_size as f64);
    thumb_spin.set_hexpand(true);
    thumb_row.append(&thumb_spin);
    content.append(&thumb_row);

    // Slideshow interval
    let slide_row = GtkBox::new(Orientation::Horizontal, 8);
    slide_row.append(&Label::new(Some("Interval slide (menit):")));
    let slide_spin = SpinButton::with_range(1.0, 120.0, 1.0);
    slide_spin.set_value(ctx.config.borrow().slideshow_interval_minutes as f64);
    slide_spin.set_hexpand(true);
    slide_row.append(&slide_spin);
    content.append(&slide_row);

    // Notifikasi toggle
    let notif_row = GtkBox::new(Orientation::Horizontal, 8);
    notif_row.append(&Label::new(Some("Notifikasi desktop:")));
    let notif_switch = Switch::new();
    notif_switch.set_active(ctx.config.borrow().notifications_enabled);
    notif_switch.set_hexpand(true);
    notif_row.append(&notif_switch);
    content.append(&notif_row);

    let save_btn = Button::with_label("Simpan");
    save_btn.add_css_class("suggested-action");
    content.append(&save_btn);

    dialog.set_child(Some(&content));

    save_btn.connect_clicked(clone!(
        #[strong] ctx,
        #[strong] dialog,
        #[strong] slideshow_btn,
        #[strong] notif_switch,
        move |_| {
            let interval_changed;
            {
                let mut cfg = ctx.config.borrow_mut();
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
                cfg.notifications_enabled = notif_switch.is_active();
                let _ = cfg.save();
            }
            reload_grid(ctx.clone(), false, None);

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
