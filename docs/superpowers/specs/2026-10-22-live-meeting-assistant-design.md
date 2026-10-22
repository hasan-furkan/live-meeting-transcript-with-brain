# Live Meeting Assistant — Design

Date: 2026-10-22
Status: approved by user (2026-10-22)

## 1. Purpose

A desktop companion for online lectures held in Microsoft Teams (works with any meeting app, since it listens to system audio). It:

- produces a live transcript of what is heard,
- notifies the user within ~2–3 s when their name (or a variant) is spoken, a keyword is heard, or a question is addressed to the class,
- on those triggers (or on a hotkey) sends the last ~350 words, plus an optional screenshot region, to an LLM and shows "what was asked / suggested answer",
- at the end of a lecture writes an English summary, notes and action items, plus the full transcript, into the user's Obsidian vault ("Beyin").

Lectures are in English. The app UI, README, notes and summaries are all in English.

Runs on Windows 11 and macOS 13+.

### Non-goals

- No Teams API, Graph API or Teams UI scraping.
- No audio recording to disk, ever.
- No automatic meeting detection; the user presses Start/Stop.
- No "questions asked to me" section in the saved notes; those analyses are live-only.

## 2. Hard constraints

- **Audio is never written to disk.** It lives only in an in-memory ring buffer (~30 s) and is dropped after transcription.
- **Screenshots are never written to disk.** They are captured in memory, encoded, sent, and dropped. Default mode is **Off**.
- **Public repository:** only application code goes to GitHub. No lecture content, transcripts, notes, settings, API keys, tokens or model files. `.gitignore` enforces this; settings and data live in OS app-data directories, never in the repo.
- **Lecture text does go to the cloud.** Audio stays local, but the last ~350 words (on every analysis), the last ~150 words (general-question classification), the whole transcript (summary) and the screenshot region (if enabled) are sent to the configured LLM provider (default OpenCode Go / DeepSeek). The user accepts this; the Settings screen states it next to the API fields.
- **API key is stored in the OS keychain** (`keyring`: Windows Credential Manager / macOS Keychain), never in a plain file or log.
- The app writes **only inside the configured notes folder**; course names are sanitised so paths cannot escape it.
- No panics in non-test code (see STANDARDS.md); every error surfaces in the UI and the app keeps running.

## 3. Stack

- **Tauri 2**: Rust core + React/TypeScript (strict) + Vite UI. All logic in Rust; the UI only displays and calls Tauri commands.
- Command/event payload types generated from Rust with `ts-rs`.
- Deliberate deviations from the default project standards: no axum (Tauri commands replace HTTP) and no sqlx/database (all state is files).

Proposed dependencies (to be approved before adding): `tauri` + plugins `notification`, `global-shortcut`, `dialog`; `whisper-rs` (feature `cuda` on Windows, `metal` on macOS); `cpal` (Windows WASAPI loopback); `screencapturekit` (macOS system audio); `xcap` (screenshots); `reqwest` (rustls); `keyring`; `serde`/`serde_json`; `tokio`; `ts-rs`; a small VAD (`webrtc-vad` or energy-based, decided in the plan).

## 4. Architecture

```
system audio ─► capture ─► stt ─┬─ fast pass (every ~2 s, last ~8 s) ─► triggers (name, keyword)
                                └─ final pass (on VAD silence)  ─► transcript (append to disk)
                                                                     │        └─► triggers (general question)
                                                                     ▼
                       triggers ─► notify (OS notification)
                                └► assist: last ~350 words + optional screenshot ─► llm ─► UI panel + notification
                       Stop ─► summarize: full transcript ─► llm ─► vault (summary file)
```

Rust modules, each with one job and testable alone:

| Module | Responsibility |
|---|---|
| `capture` | System-audio capture → 16 kHz mono f32 frames. Windows: WASAPI loopback via `cpal`. macOS: ScreenCaptureKit audio. Reconnects when the default output device changes. Also accepts a WAV file source (tests). |
| `stt` | Owns the whisper context. Fast pass: every ~2 s transcribe the last ~8 s (partial text, not persisted). Final pass: on VAD silence (or 15 s max) transcribe the segment and emit a final line. Name variants are passed as the initial prompt to bias recognition. Reports GPU/CPU mode. |
| `transcript` | Holds final lines in memory and appends each to the transcript file immediately. Provides "last N words". |
| `triggers` | Pure functions + small state: normalisation, name/keyword matching, cooldown, question heuristic. |
| `llm` | Anthropic-Messages-compatible client. Base URL, key, model from settings. Text + image content. One retry on network/5xx. Every request carries `x-opencode-session: <random id>` (one id per client instance; OpenCode Go rejects requests without it, other providers ignore it). Response may contain `thinking` blocks; only `text` blocks are used. Image input verified working on OpenCode Go / deepseek-v4.1-flash (2026-10-22). |
| `assist` | Builds the analysis request, parses the JSON answer, emits results to the UI. |
| `screenshot` | Off / Region / Full capture of the chosen monitor, in memory, PNG/JPEG encoded. |
| `vault` | Path sanitisation, file naming, frontmatter, atomic writes. |
| `summarize` | Transcript → summary markdown; chunked map-reduce if too long for the model context. |
| `settings` | JSON settings in the OS config dir + key in keychain. |

## 5. Speech-to-text

- `whisper-rs` (whisper.cpp). Windows build uses CUDA (CUDA Toolkit 12.8 present on the dev machine, RTX 5060); macOS uses Metal. If no GPU backend initialises, it falls back to CPU and the UI shows "CPU mode — slow".
- Model selectable in Settings; default **`large-v3-turbo` q5_0** (~570 MB). Full `large-v3-turbo` and smaller models also selectable. Models are downloaded on demand from the whisper.cpp Hugging Face repo into the app data dir, with progress and checksum verification.
- Language selectable; default `en`, `auto` allowed.
- **No transcription without speech.** Whisper hallucinates on silence/noise, often repeating initial-prompt words — which would fire false name notifications. Neither the fast pass nor the final pass runs unless VAD reports speech in the window being transcribed. Segments whose text equals the initial prompt, or consists only of prompt words, are discarded.
- **Name bias** (name variants as initial prompt) is a Settings toggle, default on.
- Development and automated tests use the `tiny` model.

## 6. Triggers

Normalisation for all matching: lowercase, Turkish diacritics folded (ı→i, ş→s, ğ→g, ü→u, ö→o, ç→c), punctuation removed.

| Trigger | Input | Rule | Action |
|---|---|---|---|
| Name | fast-pass partial text | Word-boundary match of any name variant. Variants ≥5 chars also match at edit distance 1; shorter ones need exact match. Cooldown 20 s per session. | Immediate notification "Your name was mentioned: …"; then assist analysis; second notification with short question/answer. |
| Hotkey / "Ask now" | user | Default `Ctrl+Shift+Space` (`Cmd+Shift+Space` on macOS), configurable. | Assist analysis + notification. |
| General question | final lines | Line ends with `?` or contains a cue phrase ("who can", "anyone", "what do you think", "can someone", …) → LLM classifies the last ~150 words: question addressed to the class expecting an answer? At most one classification per 30 s. Toggle in Settings (default on). | If yes: quiet notification + assist analysis in panel. |
| Keyword | fast-pass partial text | Same matching as names (exact only). List in Settings (e.g. homework, exam, deadline, attendance). Cooldown 20 s per keyword. | Notification only. |

Assist request: course name, last ~350 words, optional screenshot. The model must return
`{"question": "...", "suggested_answer": "...", "confidence": "low|medium|high"}`. If the reply is not valid JSON, the raw text is shown. Errors are shown in the panel; transcription continues regardless. If no API key is set, triggers still notify and the panel says "API key missing".

## 7. Vault output

Settings field **Notes folder** (no default; asked on first run), e.g. `~/Documents/<vault>/okul`. The vault's `okul/README.md` defines the layout this follows.

- Start → pick a course from the subfolders of the notes folder, or "New course…". The course name becomes the folder name after sanitisation (reject/strip `..`, path separators, control and reserved characters; trim; non-empty).
- During the lecture: `<notes>/<course>/YYYY-MM-DD-transcript.md`, appended line by line as `**[HH:MM:SS]** text`. Frontmatter:

  ```yaml
  ---
  kind: note
  project: okul
  ders: "<course>"
  tur: transcript
  tarih: YYYY-MM-DD
  visibility: private
  ---
  ```

- On Stop: `<notes>/<course>/YYYY-MM-DD-ders.md` with frontmatter `tur: ders-ozeti`, `visibility: internal` (other keys as above) and body sections `## Summary`, `## Notes`, `## Action items` (`- [ ]` checkboxes), `## Transcript` (wikilink to the transcript file).
- **Frontmatter values are escaped.** Every string value is written as a JSON-encoded string (`serde_json::to_string`), which is a valid YAML double-quoted scalar, so quotes, newlines, `---` or `: ` in a course name cannot break the block or drop keys such as `visibility: private`. (No `serde_yaml`: it is unmaintained.) Course names also have control characters and newlines stripped during sanitisation.
- A second session of the same course on the same day uses `-2`, `-3`, … suffixes on both files. Existing files are never overwritten.
- The summary file is written atomically (temp file + rename). The transcript is append-only and flushed per line, so a crash loses at most the current line.
- If summarisation fails, the transcript is intact and the UI offers **Retry summary**.

## 8. UI

Single window + tray icon. Closing the window hides it to the tray; a running session keeps listening.

- **Main:** Start/Stop, current course, live transcript (last ~20 lines), **Ask now**, analysis panel (newest first: time, question, suggested answer, confidence). Status bar: GPU/CPU mode, audio level meter with "no audio" warning, last LLM error.
- **Settings:** API base URL (default `https://opencode.ai/inference/go/anthropic`), API key, model (default `deepseek-v4.1-flash`); Whisper model + language + Download; notes folder; name variants; keywords; general-question toggle; hotkey; screenshot mode Off/Region/Full + monitor + "Select region" overlay; audio device.

## 9. Error handling

- Output device change → capture reconnects automatically.
- macOS missing Screen Recording permission → explanation + button to open System Settings.
- Corrupt/missing model → offer re-download.
- LLM errors → one retry, then shown in panel; never block transcription.
- No error path panics or exits the app.

## 10. Testing

- Tests are written by a different worker than the code, against this spec, including malicious-input cases.
- Unit: `triggers` (normalisation, fuzzy names, cooldowns, false-positive cases); `vault` (path traversal attempts, name collisions, atomic write, frontmatter; malicious course names with `"`, newlines, `---`, `visibility: public` must round-trip and keep `visibility: private`); `stt` gating (silence/noise WAV → no lines, no trigger); `llm`/`assist` (mock server: valid JSON, broken JSON, errors, retries); `summarize` chunking; `settings` (key never in JSON).
- Integration: a committed short English WAV (synthetic, no lecture content) fed through the WAV capture source with the `tiny` model → expect transcript lines and a name-trigger event.
- Quality gate `scripts/check.ps1`: cargo fmt, strict clippy, test, audit, deny + UI typecheck, lint, test, build.
- GPU backends are cargo features (`cuda`, `metal`); CI builds without `cuda` (runners have no CUDA Toolkit), local Windows release builds enable it.
- CI (GitHub Actions): gate on Windows; build + tests on macOS (macOS runtime behaviour — audio capture, permissions — is verified manually by the user on a MacBook).
