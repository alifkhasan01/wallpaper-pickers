# wallpicker

Wallpaper picker GUI (Rust + GTK4) buat Hyprland/Niri, set wallpaper lewat `awww`.

## Fitur

- **Grid thumbnail wallpaper** — di-cache di `~/.cache/wallpicker/thumbs`, pakai hash SHA-256 (path + mtime) biar auto-invalidasi kalau file berubah
- **Klik thumbnail** — langsung diterapkan lewat `awww img` dengan transisi
- **Auto-start `awww-daemon`** — kalau belum jalan
- **Search/filter** — cari by nama file (case-insensitive, real-time)
- **Slideshow** — ganti wallpaper otomatis dengan interval bisa diatur
- **Random wallpaper** — tombol shuffle di header bar
- **Ganti folder** — via file picker
- **Pengaturan lengkap** — tipe transisi, durasi, FPS, ukuran thumbnail, interval slideshow — tersimpan di `~/.config/wallpicker/config.json`
- **CLI mode** — `--random` (pilih acak) atau `--set <path>` (pilih file tertentu) tanpa GUI
- **Hyprlock integration** — nyimpen symlink di `~/.cache/wallpaper/hyprlock-bg`
- **Status bar** — nampilin progress scanning, loading, dan pesan error
- **Supported format** — jpg, jpeg, png, webp, bmp, tiff

## Config (`~/.config/wallpicker/config.json`)

| Field | Default | Deskripsi |
|---|---|---|
| `wallpaper_dir` | `~/Pictures/Wallpapers` | Folder wallpaper |
| `transition_type` | `"wipe"` | `simple`, `fade`, `wipe`, `wave`, `grow`, `center`, `outer`, `random` |
| `transition_duration` | `1.0` | Durasi transisi (detik) |
| `transition_fps` | `60` | FPS transisi |
| `thumb_size` | `220` | Ukuran thumbnail (px) |
| `slideshow_enabled` | `false` | Aktifkan slideshow |
| `slideshow_interval_minutes` | `5` | Interval slideshow (menit) |

## CLI

```bash
wallpicker                  # Buka GUI
wallpicker --random         # Set wallpaper acak, langsung keluar
wallpicker --set /path/file # Set wallpaper tertentu, langsung keluar
```

## Dependency sistem (Arch)

```bash
sudo pacman -S gtk4 awww
```

Kalau belum ada `awww` di repo resmi, install dari AUR:

```bash
yay -S awww
# atau versi git kalau mau fitur terbaru
yay -S awww-git
```

## Build

```bash
cargo build --release
```

Binary hasil build ada di `target/release/wallpicker`.

Install ke `~/.local/bin` (opsional):

```bash
install -Dm755 target/release/wallpicker ~/.local/bin/wallpicker
```

## Integrasi ke Niri

Tambahin di `~/.config/niri/config.kdl`:

```kdl
spawn-at-startup "awww-daemon"

binds {
    Mod+W { spawn "wallpicker"; }
}
```

## Integrasi ke Hyprland

Di `~/.config/hypr/hyprland.conf`:

```
exec-once = awww-daemon
bind = $mainMod, W, exec, wallpicker
```

## Catatan

- Aplikasi ini cuma bikin request ke `awww` CLI (bukan reimplement daemon-nya), jadi tetap butuh binary `awww` & `awww-daemon` ada di `$PATH`.
- Kalau mau langsung tanpa GUI popup icon di taskbar/dock, jalanin lewat keybind aja — window-nya emang biasa (bukan layer-shell), jadi bisa di-float pakai window rule kalau perlu:

  Niri:
  ```kdl
  window-rule {
      match app-id="com.wallpicker.app"
      open-floating true
  }
  ```

  Hyprland:
  ```
  windowrulev2 = float, class:^(com.wallpicker.app)$
  ```