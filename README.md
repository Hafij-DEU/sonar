# Sonar

**Bangla and English voice typing for Linux.**

Hold a shortcut, speak, let go — Sonar types what you said wherever your cursor is: a browser, a chat, a document, a terminal.

- **Two shortcuts, one per language**
  - Bangla: `Super+Alt+Space`
  - English: `Super+Alt+Shift+Space`
  - Change either one in Settings, or from the Change button on the Home screen.
- **Offline mode** — speech is recognised on your own computer. Nothing leaves it.
  - Bangla: a Bangla fine-tuned Whisper Medium model (GGUF, Q8_0, ~793 MB)
  - English: Parakeet TDT 0.6B v2 (GGUF, Q8_0)
- **Cloud mode** — uses your own Gemini (or OpenRouter) API key for higher accuracy on slower machines. Each recording is sent to the provider you choose.
- History with search and copy, Bangla or English interface, light and dark themes.

## Install

Download `Sonar_<version>_amd64.deb` from [Releases](../../releases) and install it:

```bash
sudo apt install ./Sonar_0.12.0_amd64.deb
```

The current .deb is built on Ubuntu 26.04 and needs glibc 2.43 or newer.

On first launch Sonar asks whether to work offline (download a model) or in the cloud (paste an API key). A free Gemini key is available at <https://aistudio.google.com/apikey>.

### GNOME, KDE and other Wayland desktops

On Wayland, apps cannot grab keys or type into other windows by themselves. Sonar asks the desktop through its standard portals instead:

1. **Shortcuts** — the first time Sonar starts, the desktop shows a dialog listing Sonar's two shortcuts. Allow it. To pick other keys later, use the Change button on the Home screen, or GNOME Settings → Keyboard → Keyboard Shortcuts → Apps.
2. **Typing** — the first time Sonar pastes, the desktop asks to allow *remote keyboard input*. Allow it once; Sonar keeps a token so it does not ask again until you revoke it.

Sonar always pastes on Wayland: it puts the text on the clipboard and sends Ctrl+V (or the paste keys you chose).

If a shortcut does nothing, open Sonar: the Home screen tells you when the desktop refused the shortcuts, or when the model for that language is not downloaded.

Remove it with `sudo apt remove sonar`.

## Build from source

Requirements: Rust (stable), Node.js 20+, and the Tauri 2 Linux build dependencies (WebKitGTK 4.1, GTK 3, libayatana-appindicator3, OpenSSL).

```bash
npm install
npx tauri build --bundles deb
```

The package lands in `src-tauri/target/release/bundle/deb/`. For a UI-only preview with a mocked backend, run `npx vite` and open `http://localhost:1420/sonar-preview.html` (query options: `setup=1`, `cloud=1`, `lang=bn`, `theme=dark`, `models=1`).

## Where the code is

- `src-tauri/src/sonar.rs` — language routing (Bangla/English), offline model selection, cloud transcription
- `src/sonar/` — the Sonar interface (home, history, settings, first-run setup)
- `src/i18n/locales/bn/` — Bangla interface strings

## Credits and licence

Sonar is built on [Handy](https://github.com/cjpais/Handy) by CJ Pais and contributors (MIT). Handy's recording, voice-activity detection, model management, paste and shortcut engine are used largely as-is; Sonar adds the two-language workflow, cloud mode and its own interface.

Speech models are downloaded on demand from their publishers and keep their own licences. The Bangla model is fetched from a third-party GitHub release; Sonar does not bundle any model weights except the small Silero VAD model shipped by Handy.

MIT licence — see [LICENSE](LICENSE).
