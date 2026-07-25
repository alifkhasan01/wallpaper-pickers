# wallpicker

Wallpaper picker GUI (Rust + GTK4) buat Hyprland/Niri, set wallpaper lewat `awww`.

## Fitur

- Grid thumbnail wallpaper (di-cache di `~/.cache/wallpicker/thumbs`, auto-invalidate kalau file berubah)
- Klik thumbnail → langsung diterapkan lewat `awww img` dengan transisi
- Auto-start `awww-daemon` kalau belum jalan
- Ganti folder wallpaper via file picker
- Search/filter by nama file
- Pengaturan tipe transisi, durasi, FPS, ukuran thumbnail — tersimpan di `~/.config/wallpicker/config.json`

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

Tambahin di `~/.config/niri/config.kdl` (atau bind ke keybind kalau gak mau autostart):

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
