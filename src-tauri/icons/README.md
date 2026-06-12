# Tauri Icons

This directory holds the generated app icons used by `tauri.conf.json`:

- `32x32.png` - 32×32 PNG
- `128x128.png` - 128×128 PNG
- `128x128@2x.png` - 256×256 PNG (retina)
- `icon.icns` - macOS icon bundle
- `icon.ico` - Windows icon

To regenerate all sizes from the source `icon.png` in this directory, run
`npx @tauri-apps/cli icon icons/icon.png` from `src-tauri/`.
