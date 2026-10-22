# Live Meeting Assistant

A desktop companion for online lectures. It listens to your computer's system audio, transcribes it locally with Whisper, alerts you when your name, a keyword or a question addressed to the class comes up, and — on a trigger or the **Ask now** hotkey — asks an LLM for *what was asked* and a *suggested answer*. At the end of the lecture it writes an English summary and the full transcript into your Obsidian vault. It works with any meeting app (Teams, Zoom, a browser player), since it captures what the speakers play.

## Features

- **Local transcription** of system audio with Whisper (CUDA on Windows, Metal on macOS, CPU fallback). Audio is processed in memory only.
- **Name / keyword / question alerts**: word-boundary matching of your name variants (fuzzy by one character for longer variants), keyword list, and a general-question heuristic that the LLM confirms. Cooldown per term and per feature to avoid repeats.
- **Ask now** hotkey (default `Ctrl+Shift+Space` on Windows, `Cmd+Shift+Space` on macOS) and automatic analysis on name and class-question triggers.
- **LLM analysis** of the last ~350 words (plus an optional screenshot region): the question that was asked and a short suggested answer, with a confidence level, shown in the app and as a notification.
- **Lecture notes** written into an Obsidian folder: a summary file with summary, notes and action items, plus a private full transcript.

## Privacy

- **Audio never leaves the computer and is never written to disk.** It lives in an in-memory ring buffer and is dropped after transcription.
- **Screenshots are off by default and never written to disk.** They are captured in memory and dropped after the request. Only a selected region or the full monitor is captured, and only when explicitly enabled.
- **Text does go to the LLM provider you configure** (default OpenCode Go): the last ~350 words for each analysis, the last ~150 words for general-question classification, the full transcript when producing the summary, and the screenshot region if enabled. No audio is sent.
- **The API key is stored in the OS keychain** (Windows Credential Manager / macOS Keychain), never in settings JSON or logs. Settings live in the OS app-config directory; Whisper models live in the OS app-data directory.
- **This repository contains no lecture data**, settings, transcripts, keys or model files. `.gitignore` keeps them out.

## Requirements

- **Windows 11**: NVIDIA GPU + CUDA Toolkit 12.x recommended (the release build enables CUDA); works on CPU without them.
- **macOS 13+** on Apple Silicon (Metal). Grant **Screen Recording** permission in System Settings → Privacy & Security for system audio capture and screenshots; without it the app reports a permission error.
- **Rust** stable (1.90 or newer) and the **Tauri CLI** (`cargo install tauri-cli --version ^2`).
- **Node 22**.
- **CMake** and **libclang** for building whisper-rs.
  - Windows: `pip install --user libclang`, or install LLVM.
  - macOS: Xcode command line tools (`xcode-select --install`).

## Build & run

```sh
npm --prefix ui ci

# Windows: scripts/build.ps1 finds CMake (also inside Visual Studio) and libclang for you
powershell -File scripts/build.ps1        # release bundle (.msi)
powershell -File scripts/build.ps1 dev    # development

# macOS (Xcode command line tools provide clang and the toolchain)
cargo tauri build --features metal        # .app/.dmg
cargo tauri dev --features metal
```

On Windows, a plain `cargo tauri build` fails with "Unable to find libclang" unless `LIBCLANG_PATH` (and CMake on `PATH`) are set; `scripts/env.ps1` sets them.

Quality gate (format, clippy, tests, audit, deny, UI typecheck/lint/test/build):

```powershell
powershell -File scripts/check.ps1
```

The gate must end with `CHECK PASSED`.

## First run

1. Open **Settings** and set the API base URL, API key and model. Defaults are `https://opencode.ai/inference/go/anthropic` and `deepseek-v4.1-flash`; any Anthropic-Messages-compatible endpoint works.
2. Download the Whisper model (default `large-v3-turbo-q5_0`) with the **Download** button. Models are fetched from the whisper.cpp repository on Hugging Face and checksum-verified.
3. Choose your **notes folder**, e.g. `<vault>/okul`. Courses are the subfolders of this folder.
4. Add your **name variants**, adjust the **keywords** (defaults: homework, assignment, exam, quiz, deadline, attendance) and optionally the **hotkey**. Optionally set an **explanation language** (e.g. Turkish, Deutsch) to get a 1–2 sentence explanation under each suggested answer; leave it empty to turn it off.
5. Choose a **screenshot mode**: Off (default), Region (pick a screen region) or Full, and the monitor to capture.
6. Pick a course and press **Start**. The app listens while the window is hidden in the tray; closing the window does not stop a session.

## Vault output

For course `Algorithms 101` on 2026-10-22, a session writes:

```
<notes>/Algorithms 101/2026-10-22-ders.md          # summary, visibility: internal
<notes>/Algorithms 101/2026-10-22-transcript.md    # full transcript, visibility: private
```

A second session of the same course on the same day gets a `-2` suffix on both files (`-3`, … after that). Existing files are never overwritten.

Summary frontmatter:

```yaml
---
kind: note
project: okul
ders: "Algorithms 101"
tur: ders-ozeti
tarih: 2026-10-22
visibility: internal
---
```

The body has `## Summary`, `## Notes`, `## Action items` and a `## Transcript` wikilink to the transcript file. The transcript uses the same frontmatter with `tur: transcript` and `visibility: private`, and appends lines as `**[HH:MM:SS]** text` while the lecture runs.

## Project layout

- `core/` — Rust logic: audio capture, Whisper STT, triggers, LLM client, screenshots, vault writing, summarisation.
- `src-tauri/` — Tauri app: commands, events, tray, notifications, global hotkey.
- `ui/` — React + TypeScript UI (main view, settings, region picker).
- `scripts/` — `check.ps1` quality gate and test-fixture helper.
- `docs/` — design spec and implementation plan.

## License

MIT. See [LICENSE](LICENSE).
