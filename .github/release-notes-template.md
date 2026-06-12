# Power Interview AI {{TAG}}

## What's changed
{{CHANGES}}

## Installation
- **Windows**: download the `.exe` (NSIS) installer. It installs per-user (no admin rights required).
- **macOS**: download the `.dmg` (universal binary, runs on Apple Silicon and Intel).

The app checks for and installs future updates automatically.

> **macOS first launch**: this build is not notarized by Apple, so Gatekeeper will block
> the first launch ("app is damaged" or "cannot be opened"). After dragging the app to
> Applications, either right-click it and choose **Open** (then confirm), or run:
> ```
> xattr -dr com.apple.quarantine "/Applications/Power Interview AI.app"
> ```

## Having trouble installing?

Just use the command line.

### Windows
```
curl -L -o "{{WIN_ASSET}}" https://github.com/{{REPO}}/releases/latest/download/{{WIN_ASSET}} && start "" "{{WIN_ASSET}}"
```

### macOS
```
curl -L -o "{{MAC_ASSET}}" https://github.com/{{REPO}}/releases/latest/download/{{MAC_ASSET}} && open "{{MAC_ASSET}}"
```
