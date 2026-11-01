# Translate

English to Korean chat translation for WoW: Forever and retail World of Warcraft, forked from [cherryroots/Translate](https://github.com/cherryroots/Translate) (Chinese to English). It comes in two parts:

- **`addon/Translate`**: the in-game addon. One folder loads in both clients (`## Interface: 16001, 120105`).
- **`companion`**: a small Rust app that runs next to the game, translates through any OpenAI-compatible API, and draws the Korean over your chat box.

WoW addons cannot reach the internet, and WoW only writes its chat log at logout, so the addon hands new lines to the companion through the screen and the clipboard. Nothing presses keys for you.

## How it works

1. **Automatic:** when an English line from another player arrives, the addon draws it as a small strip of colored blocks near the top-left corner of the screen (256 x 32 pixels). The companion finds the strip on screen, decodes it, translates the line and shows the Korean in a see-through, click-through overlay that you place over your chat frame.
2. **Click to translate, inside the game:** turn on the blue `[T]` link after English lines with `/tr links`. Clicking it opens a small window with the line already highlighted. Copy it, wait for the overlay to say the line is ready, then click the lower box and paste. The Korean is printed in chat under a `[KO]` tag.
3. **Replying:** type Korean in the companion's reply box and press Enter. The English is put on your clipboard, ready to paste into WoW chat. Without leaving the game, type `/tr 멈춰` instead (`/tr zh 멈춰` for Chinese; `/ㅅㄱ` and `/번역` are the same command, for when the Korean IME is still on): the line goes through the strip, the overlay shows the translation when it is on the clipboard, and you paste it into chat.

Reading the strip is screen capture: it only reads pixels and never sends input to the game, but Blizzard's terms do not clearly allow or forbid it. Turn the strip off with `/tr strip` to rely on the click-to-translate route alone.

## Download

Get everything from the [latest release](https://github.com/cherryroots/Translate/releases/latest):

| File | What it is |
| --- | --- |
| `Translate-addon.zip` | The addon, for WoW: Forever and retail |
| `translate-companion-windows-x64.exe` | Companion app for Windows |
| `translate-companion-macos-universal` | Companion app for macOS (Apple silicon and Intel) |
| `translate-companion-linux-x64` | Companion app for Linux |

## Install the addon

Unzip `Translate-addon.zip` into the AddOns folder of the client you play, so the files end up in a `Translate` folder:

```
Forever:      World of Warcraft\_forever_\Interface\AddOns\Translate\
Retail:       World of Warcraft\_retail_\Interface\AddOns\Translate\
```

Restart the client if the addon does not appear in the list. If retail lists it as out of date, the retail patch has moved past `120105`: check the number with `/dump select(4, GetBuildInfo())` and update the `## Interface:` line. In game:

| Command | Does |
| --- | --- |
| `/tr` | Open the window on the latest English line (also use the `<` `>` buttons) |
| `/tr strip` | Toggle the pixel strip the companion reads |
| `/tr links` | Toggle the `[T]` links |
| `/tr <Korean>` | Put the English translation on the clipboard; `/tr zh <Korean>` for Chinese |
| `/tr trade` | Toggle translating the Trade channel |
| `/tr strippos <x> <y>` | Move the strip, in screen pixels from the top-left corner. On macOS it starts 48 pixels down, below the menu bar |
| `/tr test` | Send a test line through the strip and overlay, without saying anything in chat |
| `/tr probe` | Report which client features the addon found |

## Run the companion

Download the companion for your system from the [latest release](https://github.com/cherryroots/Translate/releases/latest) and put it in its own folder, since it writes `config.json` next to itself. Builds of unreleased changes are under the **Actions** tab (open the latest *Build* run and look under Artifacts). You can also build it yourself with `cargo build --release` inside `companion/` (Linux also needs the PipeWire, X11 and Wayland development packages listed in `.github/workflows/build.yml`).

On macOS and Linux, browsers drop the execute permission, so run `chmod +x translate-companion-*` once. The macOS build is also unsigned: run `xattr -d com.apple.quarantine translate-companion-macos-*` or right-click it and choose Open. macOS will also ask for Screen Recording permission so the companion can read the strip.

On first start it writes `config.json` next to the program. Open **Settings** in the app to set the **API base URL / key / model**: any OpenAI-compatible endpoint. A bare address like `http://localhost:1234` gets `/v1` added.

- **Default:** Tencent's translation model [HY-MT1.5-7B](https://huggingface.co/tencent/HY-MT1.5-7B-GGUF) on a local [Ollama](https://ollama.com) server: run `ollama pull hf.co/tencent/HY-MT1.5-7B-GGUF:Q4_K_M` once (4.6 GB). It is free and works offline.
- **LM Studio:** load the same GGUF, start the server, and use `http://localhost:1234/v1` with the model name LM Studio shows.
- **OpenAI:** `https://api.openai.com/v1` with your key and a model such as `gpt-4o-mini`.

When the model name contains `hy-mt`, the companion switches to HY-MT's own prompt and sampling settings from its model card, and passes the WoW abbreviations found in each line (LFM, PUG, PST, BiS, MT...) as terminology hints. Other models get a system prompt that explains the slang instead. The list is `SLANG` in `companion/src/translate.rs`: `en=ko` pairs, matched as whole words in any case.

**Test model** sends a fixed English line to the configured model and shows the answer and how long it took. The window also says whether it has found the pixel strip.

Run WoW in **windowed** or **windowed (fullscreen)** mode; overlays cannot draw over exclusive fullscreen, and screen capture may see a black screen there. Click **Unlock overlay** to drag it over your chat frame, then **Lock** it so clicks pass through to the game. While locked, the overlay hides whenever another window is in front of WoW; untick **Hide when WoW is in the background** to keep it always visible. Keep the overlay and the macOS menu bar off the strip.

To debug the strip, take a screenshot and run `translate-companion --decode screenshot.png`.

## Known limits

- Busy channels such as Trade can produce lines faster than a slow model translates them, and the overlay then falls behind.
- Chat text inside instances may be a secret value under Midnight rules. The addon cannot read those lines, so they are not translated.
- Each strip frame holds 376 bytes, which fits any WoW chat message (255 bytes at most) plus the channel and name. Longer text would be cut at a character boundary.
- The strip assumes the UI is drawn at the screen's physical resolution. If the companion never finds it, send a screenshot through `--decode`.

## Icon

`assets/icon.svg` is the source for both icons. `companion/icon.png` is the window icon, `companion/icon.ico` is built into the Windows exe, and `addon/Translate/icon.tga` is shown in the AddOns list through `## IconTexture`. To change the icon, edit the SVG and re-export those three files at the same sizes.

## Making a release

Run the **Build** workflow from the Actions tab with a tag such as `v0.2.0` in the *release* box, or push a `v*` tag. The workflow builds everything and publishes the release with notes generated from the merged pull requests. The tag sets the version: a `v0.2.0` run first commits "Release v0.2.0" to main with `## Version` in `Translate.toc` and the companion version in `companion/Cargo.toml` and `Cargo.lock` set to 0.2.0, then builds and tags that commit. There is nothing to bump by hand. Prefer the release box: a pushed tag stays on the commit you tagged, before the version commit.

## License

MIT, see [LICENSE](LICENSE). Contributions are welcome: see [CONTRIBUTING.md](CONTRIBUTING.md).
