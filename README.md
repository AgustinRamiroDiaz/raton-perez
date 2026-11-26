# 🐭 Ratón Pérez

Speak, then polish. Ratón Pérez is a desktop dictation app: press a shortcut, speak, and your words are transcribed locally. Before pasting, a writing-tools popup lets you proofread, rewrite, change the tone, summarize, extract key points or build a table with an LLM.

It is a fork of [Handy](https://github.com/cjpais/Handy) (local speech-to-text) with a popup inspired by [WritingTools](https://github.com/theJayTea/WritingTools).

The name comes from our initials (T, R, A → RAT) and from _El Ratón Pérez_, the Argentine tooth fairy. Teeth help us speak.

## What it does

- **Local transcription.** Audio is transcribed on your machine with Whisper or Parakeet models. Nothing is sent to the cloud for speech-to-text.
- **Writing-tools popup.** After dictating, the popup shows your transcript in an editable **Original** box and the result in an editable **Final** box. Every action reads Original and replaces Final, so actions don't stack.
- **Preset actions.** Proofread, Rewrite, Friendly, Professional, Concise, Summary, Key Points and Table, plus a free-text custom instruction.
- **Local LLM by default.** Actions run through [Ollama](https://ollama.com) with `llama3.2:3b`. You can switch to OpenAI, Anthropic, Groq, OpenRouter, Cerebras, Z.AI, AWS Bedrock, Apple Intelligence (Apple Silicon Macs) or a custom OpenAI-compatible endpoint.
- **Plain dictation still works.** The original Handy shortcuts paste straight into the focused text field, with or without automatic cleanup.

## Requirements

| Requirement                                     | Why                                          |
| ----------------------------------------------- | -------------------------------------------- |
| [Ollama](https://ollama.com/download)           | Runs the default LLM for the popup actions   |
| [Rust](https://rustup.rs/) (latest stable)      | Builds the app                               |
| [Bun](https://bun.sh/)                          | Installs and runs the frontend               |
| Platform build tools (see [BUILD.md](BUILD.md)) | Xcode Command Line Tools on macOS, and so on |

Ollama is only needed for the LLM actions. If you choose a cloud provider in settings, or the [hackathon proxy](#hackathon-use-the-hosted-bedrock-proxy-no-ollama), you can skip it.

## Installation

There may be no prebuilt installer; check the [Releases page](https://github.com/AgustinRamiroDiaz/raton-perez/releases) first. Otherwise run it from source.

### 1. Install the build tools

macOS:

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
curl -fsSL https://bun.sh/install | bash
```

Windows and Linux need extra system packages. They are listed in [BUILD.md](BUILD.md).

### 2. Install Ollama and pull the default model

Install Ollama from <https://ollama.com/download>, then:

```bash
ollama pull llama3.2:3b
```

The Ollama desktop app starts its server automatically. If you only have the CLI, run `ollama serve`.

### 3. Clone and run

```bash
git clone https://github.com/AgustinRamiroDiaz/raton-perez
cd raton-perez
bun install
bun run tauri dev
```

The first run compiles the Rust side and takes a few minutes. To produce an installable bundle (`.dmg`, `.msi`, `.deb`, AppImage) instead, run `bun run tauri build`.

### 4. First launch

1. Grant microphone and accessibility permissions when the system asks.
2. Download a speech model when the app asks.

## Using it

1. Press the writing-tools shortcut, speak, then press it again.
2. The popup opens with your transcript in **Original**. Edit it if needed.
3. Click an action or type a custom instruction. The result appears in **Final**, which you can also edit.
4. Click **Paste** to insert the text into the app you were using, or **Copy** to put it on the clipboard.

### Default shortcuts

| Action                                      | macOS              | Windows          | Linux                |
| ------------------------------------------- | ------------------ | ---------------- | -------------------- |
| Transcribe to writing tools (popup)         | Option+Shift+R     | Ctrl+Alt+R       | Ctrl+Alt+R           |
| Transcribe and paste                        | Option+Space       | Ctrl+Space       | Ctrl+Alt+Space       |
| Transcribe with automatic cleanup and paste | Option+Shift+Space | Ctrl+Shift+Space | Ctrl+Alt+Shift+Space |
| Cancel                                      | Escape             | Escape           | Escape               |

All shortcuts can be changed in Settings.

The same actions can be triggered from a terminal while the app runs, which is handy for scripts and global shortcuts the app can't register (for example on Wayland). With a development build, the binary is `src-tauri/target/debug/handy`:

```sh
handy --toggle-popup          # start, then stop, a recording for the writing-tools popup
handy --toggle-post-process   # same, with automatic cleanup and paste
handy --toggle-transcription  # same, plain transcription
handy --cancel
```

### Scripted demo (Linux, X11)

`scripts/demo.sh` plays a synthetic voice into a virtual microphone and drives the app and the popup with the flags above and the popup's keyboard shortcuts: a dictation cleaned up by the LLM, popup actions with a custom instruction, and notes turned into key points and a table, each pasted into a text editor. It needs `xdotool`, `pactl`/`paplay` and [`uv`](https://docs.astral.sh/uv/) (for Piper TTS), and post-processing set up in the app. Quit the app first, then:

```sh
bun run tauri build --debug --no-bundle   # a dev build with the frontend embedded
scripts/demo.sh --record demo.mp4         # --record also needs ffmpeg and parecord
```

The script takes over the keyboard and mouse for about two minutes. See the top of the script for the timing variables.

### Changing the model or provider

Open **Settings → Post-Processing**. With the "Ollama (local)" provider, the page lists the models you have installed in Ollama. Cloud providers need an API key. The preset prompts can be edited on the same page.

### Hackathon: use the hosted Bedrock proxy (no Ollama)

During the Particle41 hackathon, team-4 runs an OpenAI-compatible endpoint backed by AWS Bedrock, so you can try the popup actions without installing a local model. In **Settings → Post-Processing**, choose the **Custom** provider and set:

| Field    | Value                                              |
| -------- | -------------------------------------------------- |
| Base URL | `https://raton-perez.dev.apps.particle41.ninja/v1` |
| API key  | ask team-4 in the hackathon Slack channel          |
| Model    | `claude`                                           |

The key is shared by every hackathon participant, so don't post it publicly or commit it. Team members can read it from the cluster; the steps are in the proxy's repository, [Particle41/raton-perez-proxy](https://github.com/Particle41/raton-perez-proxy#reading-the-api-key) (Particle41 members only).

## Common issues and workarounds

### The popup actions fail or return nothing

The default provider talks to Ollama at `http://localhost:11434`. Check that:

- Ollama is running (`ollama list` should respond).
- The model is installed (`ollama pull llama3.2:3b`), or pick an installed one in **Settings → Post-Processing**.

### macOS build fails with `tapi error: malformed file ... unknown architecture`

```text
ld: tapi error: malformed file
/Library/Developer/CommandLineTools/SDKs/MacOSX27.0.sdk/usr/lib/libSystem.B.tbd:4:20: error: unknown architecture
```

The linker in your Command Line Tools is older than the SDK it is pointed at. This is a toolchain problem on the Mac, not in this project.

Workaround: build against an older SDK if you have one.

```bash
ls /Library/Developer/CommandLineTools/SDKs/
export SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.sdk   # use a name from the list
cargo clean --manifest-path src-tauri/Cargo.toml
bun run tauri dev
```

If only the newest SDK is installed, update or reinstall the Command Line Tools (`softwareupdate --list`, or delete `/Library/Developer/CommandLineTools` and run `xcode-select --install`), or install a matching version from <https://developer.apple.com/download/all/>.

### App panics on startup with `NotFound` at `src/lib.rs` (tray icon)

```text
thread 'main' panicked at src/lib.rs:262:14:
called `Result::unwrap()` on an `Err` value: Io(Os { code: 2, kind: NotFound, ... })
```

The app could not find its tray icon in `src-tauri/target/debug/resources/`. This has been seen with very recent Rust toolchains that use a new build-directory layout (paths like `target/debug/build/<crate>/<hash>/out`), where the resources end up in `target/debug/build/resources/` instead.

Workaround: link the resources to where the app looks for them, then run again.

```bash
rm -rf src-tauri/target/debug/resources
ln -s "$PWD/src-tauri/resources" src-tauri/target/debug/resources
bun run tauri dev
```

Recreate the link after any `cargo clean`.

### macOS shows "handy quit unexpectedly"

This is the system crash report, not a permission prompt. The development binary is still named `handy`. Click **OK** and read the terminal output for the actual error.

### macOS accessibility stays on "Waiting..." after a rebuild

Local builds are ad-hoc signed, so a rebuild can have a new code identity while the old entry in **System Settings → Privacy & Security → Accessibility** still looks enabled. Remove the old entry and grant the permission again. [BUILD.md](BUILD.md#troubleshooting) has the full steps.

### The shortcut does nothing

- On macOS, check that both microphone and accessibility permissions are granted.
- Another app may already use the shortcut. Change it in Settings.
- On Linux with Wayland, global shortcuts must be bound in your desktop environment. See the Handy notes linked below.

### Other platform issues

Ratón Pérez inherits Handy's platform behaviour for pasting, Wayland, overlays and Windows administrator apps. For those, see the [Handy README](https://github.com/cjpais/Handy#readme) and [Handy issues](https://github.com/cjpais/Handy/issues). Note that Ratón Pérez keeps its data in its own folder (`~/Library/Application Support/com.ratonperez.app/` on macOS), not Handy's.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and [BUILD.md](BUILD.md). Fixes that are not specific to the writing-tools popup are usually better sent to [Handy](https://github.com/cjpais/Handy) upstream.

## License and credits

MIT License. See [LICENSE](LICENSE).

- [Handy](https://github.com/cjpais/Handy) by CJ Pais, which this project is forked from. The Handy name, logo and brand assets are not part of the open-source license; Ratón Pérez is an unofficial fork and is not affiliated with or endorsed by Handy.
- [WritingTools](https://github.com/theJayTea/WritingTools) for the popup idea.
- Whisper by OpenAI, ggml, Silero VAD and Tauri.
