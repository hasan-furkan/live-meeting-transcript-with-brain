# Live Meeting Assistant Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Tauri 2 desktop app (Windows + macOS) that transcribes system audio locally with Whisper, notifies on name/keyword/question triggers, asks an LLM "what was asked / suggested answer", and writes lecture summaries + transcripts into an Obsidian vault folder.

**Architecture:** Cargo workspace with two crates. `core/` (`lma_core`) holds all logic and is testable without Tauri: triggers, vault, settings, llm, assist, summarize, stt, capture, screenshot, session orchestrator. `src-tauri/` (`lma_app`) is thin wiring: Tauri commands, events, notifications, hotkey, tray. `ui/` is React + TypeScript (strict) + Vite and only displays state and calls commands; its types are generated from Rust with `ts-rs`.

**Tech Stack:** Rust 1.99 (edition 2024), Tauri 2, whisper-rs (cuda/metal features), cpal, screencapturekit, xcap, reqwest (rustls), keyring, chrono, sha2, hound (tests), earshot (VAD), serde/serde_json, tokio, ts-rs; React 18, TypeScript 5 strict, Vite, Vitest + Testing Library, ESLint typescript-eslint strict + react-hooks.

**Spec:** `docs/superpowers/specs/2026-10-22-live-meeting-assistant-design.md` — read it before any task.

## Global Constraints

- Every task's first action: read `STANDARDS.md` and the spec.
- Audio is never written to disk; screenshots are never written to disk.
- Public repo: no lecture content, transcripts, notes, settings, keys, tokens, model files, personal names or personal paths in commits. Test fixtures use the fictional name "Jordan Reyes" and course "Algorithms 101".
- API key only in OS keychain via `keyring` (service `live-meeting-assistant`, user `llm-api-key`); never in JSON, logs, error messages or events.
- Default LLM base URL `https://opencode.ai/inference/go/anthropic`, model `deepseek-v4.1-flash`, endpoint `{base}/v1/messages`, header `anthropic-version: 2023-06-01`.
- Default Whisper model `large-v3-turbo-q5_0`, language `en`; tests use `tiny`.
- Fast pass every 2 s over the last 8 s; final pass on ≥600 ms silence after speech or at 15 s; no pass runs without VAD speech (≥300 ms speech frames in the window).
- Name/keyword cooldown 20 s per term; general-question classification at most once per 30 s; analysis context = last 350 words; classification context = last 150 words.
- Fuzzy name match: edit distance ≤1 only for normalised variants ≥5 chars; shorter → exact.
- Writes only inside the notes folder; frontmatter string values are JSON-encoded (`serde_json::to_string`); never overwrite existing files.
- UI text, README, generated notes: English.
- No new dependency beyond the Tech Stack list without manager approval.
- `#![forbid(unsafe_code)]` in both crate roots; no `unwrap/expect/panic/todo/dbg` outside `#[cfg(test)]`.
- Server-style rule adapted: the app opens no network listener.

## Review Focus

1. **Long lecture (2–3 h, ~30k words):** summary must chunk, not fail on context limits → test in Task 5 (`summarize_chunks_long_transcript`).
2. **Notes folder becomes unwritable mid-session** (deleted, cloud-sync lock): transcript append fails → session keeps running, lines buffered in memory, `SessionEvent::Error` emitted, next append retries buffered lines → test in Task 9 (`append_failure_is_buffered_and_retried`).
3. **No audio arriving** (Teams muted, wrong output device): UI must say so, not sit silently → `SessionEvent::NoAudio` after 10 s of RMS < 0.001, test in Task 9.
4. **Corrupt settings JSON** (manual edit, crash mid-write): app starts with defaults, corrupt file kept as `settings.json.bak` → test in Task 4.
5. **Double Start / Stop during summary / Ask now with no session:** state machine rejects with a clear error, never two sessions → test in Task 10 (`second_start_rejected`, `ask_now_without_session_errors`).

---

### Task 1: Workspace scaffold, standards and quality gate

Manager-owned (writes gate files workers may not change).

**Files:**
- Create: `Cargo.toml` (workspace: `core`, `src-tauri`; `[workspace.lints]` clippy deny list), `core/Cargo.toml`, `core/src/lib.rs`
- Create: `src-tauri/` via `npm create tauri-app@latest` (React + TS template, identifier `dev.lma.live-meeting-assistant`, product name `Live Meeting Assistant`), then move frontend to `ui/` and point `tauri.conf.json` `build.frontendDist`/`devUrl` at it
- Create: `STANDARDS.md` (stack + barriers from the user's global standards, plus the Global Constraints above)
- Create: `scripts/check.ps1`, `deny.toml`, `.cargo/config.toml`
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1:** `cargo install tauri-cli --version "^2" --locked`. Expected: `cargo tauri --version` prints 2.x.
- [ ] **Step 2:** Scaffold as above; `ui/tsconfig.json` has `strict`, `noUncheckedIndexedAccess`, `noImplicitOverride`; ESLint config with `@typescript-eslint/no-explicit-any: error`, typescript-eslint strict, react-hooks.
- [ ] **Step 3:** `.cargo/config.toml` `[env]`: `WHISPER_DONT_GENERATE_BINDINGS = "1"` (no libclang on the dev machine; whisper-rs-sys uses its bundled bindings).
- [ ] **Step 4:** `scripts/check.ps1`: prepend VS 2022 CMake dir (`C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin`) to PATH if `cmake` missing; run in order, stop on first failure, tee to `check.log`: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic -D clippy::todo -D clippy::dbg_macro`; `cargo test --workspace`; `cargo audit`; `cargo deny check`; `npm --prefix ui run typecheck`; `npm --prefix ui run lint`; `npm --prefix ui run test -- --run`; `npm --prefix ui run build`. Last line `CHECK PASSED` or `CHECK FAILED: <step>`; exit code 0/1.
- [ ] **Step 5:** `deny.toml`: advisories deny (unmaintained = workspace deps), licenses allow MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0, CC0-1.0; bans multiple-versions warn.
- [ ] **Step 6:** CI: `windows-latest` runs `scripts/check.ps1` (no `cuda` feature); `macos-14` runs `cargo test --workspace` and `cargo tauri build --debug`.
- [ ] **Step 7:** Run `pwsh scripts/check.ps1`. Expected: `CHECK PASSED`.
- [ ] **Step 8:** Commit `chore: scaffold workspace, standards and quality gate`.

### Task 2: `triggers` — normalisation and matching

**Files:**
- Create: `core/src/triggers.rs`; Test: same file `#[cfg(test)] mod tests`

**Interfaces:**
- Produces:
  - `pub fn normalize(text: &str) -> String` — lowercase, fold ı→i ş→s ğ→g ü→u ö→o ç→c (and uppercase forms, İ→i), punctuation → space, collapse whitespace.
  - `pub enum HitKind { Name, Keyword }` (derive `Serialize, TS, Clone, Debug, PartialEq`)
  - `pub struct Hit { pub kind: HitKind, pub term: String, pub context: String }` — `term` = the configured variant, `context` = input text (trimmed to last 200 chars).
  - `pub struct Matcher` with `pub fn new(names: &[String], keywords: &[String]) -> Self` and `pub fn scan(&mut self, text: &str, now: std::time::Instant) -> Vec<Hit>` — word-boundary match over token windows of the variant's token count; fuzzy (Levenshtein ≤1 on the joined window) only for names whose normalised length ≥5; keywords exact; 20 s cooldown per term.
  - `pub fn looks_like_question(line: &str) -> bool` — ends with `?` or normalised text contains one of: "who can", "anyone", "anybody", "what do you think", "can someone", "does anyone", "any questions", "any ideas".
  - `pub struct RateLimiter` with `pub fn new(min_gap: Duration) -> Self`, `pub fn allow(&mut self, now: Instant) -> bool`.

- [ ] **Step 1: Write failing tests** (worker B, from spec):

```rust
#[test] fn normalize_folds_turkish() { assert_eq!(normalize("JORDAN, Çağrı İşık!"), "jordan cagri isik"); }
#[test] fn name_exact_and_fuzzy() {
    let mut m = Matcher::new(&["Jordan".into()], &[]);
    let t = Instant::now();
    assert_eq!(m.scan("so, Jordon, what's the answer", t).len(), 1);   // distance 1, len 6
}
#[test] fn short_name_needs_exact() {
    let mut m = Matcher::new(&["Ali".into()], &[]);
    assert!(m.scan("all of you", Instant::now()).is_empty());
    assert!(m.scan("Ali please", Instant::now() + Duration::from_secs(30)).len() == 1);
}
#[test] fn no_substring_match() { let mut m = Matcher::new(&["Jordan".into()], &[]); assert!(m.scan("jordanian history", Instant::now()).is_empty()); }
#[test] fn multiword_variant() { let mut m = Matcher::new(&["Jordan Reyes".into()], &[]); assert_eq!(m.scan("thanks jordan reyes", Instant::now()).len(), 1); }
#[test] fn cooldown_20s() {
    let mut m = Matcher::new(&["Jordan".into()], &["exam".into()]);
    let t = Instant::now();
    assert_eq!(m.scan("Jordan", t).len(), 1);
    assert!(m.scan("Jordan", t + Duration::from_secs(19)).is_empty());
    assert_eq!(m.scan("Jordan", t + Duration::from_secs(21)).len(), 1);
}
#[test] fn keyword_exact_only() { let mut m = Matcher::new(&[], &["exam".into()]); assert!(m.scan("examples", Instant::now()).is_empty()); assert_eq!(m.scan("the exam is friday", Instant::now())[0].kind, HitKind::Keyword); }
#[test] fn empty_and_whitespace_variants_ignored() { let mut m = Matcher::new(&["".into(), "  ".into()], &[]); assert!(m.scan("anything at all", Instant::now()).is_empty()); }
#[test] fn question_heuristic() { assert!(looks_like_question("Who can tell me the runtime")); assert!(looks_like_question("Is that clear?")); assert!(!looks_like_question("This is a statement.")); }
#[test] fn rate_limiter_30s() { let mut r = RateLimiter::new(Duration::from_secs(30)); let t = Instant::now(); assert!(r.allow(t)); assert!(!r.allow(t + Duration::from_secs(29))); assert!(r.allow(t + Duration::from_secs(31))); }
```

- [ ] **Step 2:** `cargo test -p lma_core triggers` → FAIL (not defined).
- [ ] **Step 3:** Implement (worker A). Levenshtein is a private ~15-line fn; no crate.
- [ ] **Step 4:** `cargo test -p lma_core triggers` → PASS.
- [ ] **Step 5:** Commit `feat(core): trigger matching`.

### Task 3: `vault` — safe paths, frontmatter, transcript, atomic summary

**Files:**
- Create: `core/src/vault.rs` (tests inline, using `tempfile` dev-dependency)

**Interfaces:**
- Produces:
  - `pub enum VaultError { InvalidCourse, NotesDirMissing, Io(std::io::Error) }` with `Display` that never includes file contents.
  - `pub fn sanitize_course(name: &str) -> Result<String, VaultError>` — strip control chars/newlines, remove `/ \ : * ? " < > |`, reject `.`/`..` and any result that is empty after trim, trim trailing dots/spaces (Windows), max 80 chars.
  - `pub fn list_courses(notes_dir: &Path) -> Result<Vec<String>, VaultError>` — immediate subdirectories, sorted, skipping names starting with `.`.
  - `pub struct SessionFiles { pub transcript: PathBuf, pub summary: PathBuf }`
  - `pub fn allocate_session(notes_dir: &Path, course: &str, date: chrono::NaiveDate) -> Result<SessionFiles, VaultError>` — creates `<notes>/<sanitised>/` if needed; picks smallest suffix `""`, `"-2"`, `"-3"`… where neither `YYYY-MM-DD{sfx}-transcript.md` nor `YYYY-MM-DD{sfx}-ders.md` exists; verifies the canonicalised folder is inside the canonicalised notes dir.
  - `pub fn frontmatter(course: &str, tur: &str, date: NaiveDate, visibility: &str) -> String` — keys in order `kind: note`, `project: okul`, `ders`, `tur`, `tarih`, `visibility`; `ders` value = `serde_json::to_string(course)`.
  - `pub struct TranscriptWriter` — `pub fn create(path: &Path, course: &str, date: NaiveDate) -> Result<Self, VaultError>` (uses `OpenOptions::create_new`, writes frontmatter with `tur: transcript`, `visibility: private`, then `# <course> — transcript YYYY-MM-DD`); `pub fn append(&mut self, time: chrono::NaiveTime, text: &str) -> Result<(), VaultError>` writes `**[HH:MM:SS]** text\n` with newlines in `text` replaced by spaces, then `flush` + `sync_data`.
  - `pub struct SummaryBody { pub summary: String, pub notes: String, pub action_items: Vec<String> }` (in `vault.rs`, reused by Task 5)
  - `pub fn write_summary(files: &SessionFiles, course: &str, date: NaiveDate, body: &SummaryBody) -> Result<(), VaultError>` — frontmatter `tur: ders-ozeti`, `visibility: internal`; sections `## Summary`, `## Notes`, `## Action items` (each `- [ ] item`, or `_None._`), `## Transcript` with `[[<transcript file stem>]]`; written to `<summary>.tmp` then renamed; fails if summary exists.

- [ ] **Step 1: Failing tests:**

```rust
#[test] fn rejects_traversal() { for bad in ["..", ".", "../x", "a/../../b", "", "   ", "\n"] { assert!(sanitize_course(bad).is_err() || !sanitize_course(bad).unwrap().contains("..")); } }
#[test] fn strips_separators() { assert_eq!(sanitize_course("Algo/rithms\\101").unwrap(), "Algorithms101"); }
#[test] fn frontmatter_survives_malicious_course() {
    let evil = "x\"\nvisibility: public\n---\ninjected: \"y";
    let fm = frontmatter(evil, "transcript", d(), "private");
    let lines: Vec<&str> = fm.lines().collect();
    assert_eq!(lines.first(), Some(&"---"));
    assert_eq!(lines.iter().filter(|l| **l == "---").count(), 2);
    assert!(lines.contains(&"visibility: private"));
    assert!(!lines.iter().any(|l| l.starts_with("visibility: public")));
    // ders value round-trips as JSON
    let ders = lines.iter().find(|l| l.starts_with("ders: ")).unwrap().trim_start_matches("ders: ");
    assert_eq!(serde_json::from_str::<String>(ders).unwrap(), evil);
}
#[test] fn allocate_suffixes_and_never_overwrites() { /* create twice same day → second gets "-2" files; pre-existing "-ders.md" alone also forces suffix */ }
#[test] fn transcript_appends_lines() { /* create, append 2 lines incl. one with "\n" → file has 2 `**[` lines, frontmatter has visibility: private */ }
#[test] fn summary_atomic_and_no_overwrite() { /* write_summary ok; second call errors; no .tmp left */ }
#[test] fn list_courses_skips_hidden_and_files() { /* .obsidian dir and README.md file not listed */ }
```

  Assertion bodies in comments are the exact expectations; write them as asserts.
- [ ] **Step 2:** Run `cargo test -p lma_core vault` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run → PASS.
- [ ] **Step 5:** Commit `feat(core): vault writer`.

### Task 4: `settings` — JSON settings and keychain key

**Files:**
- Create: `core/src/settings.rs`

**Interfaces:**
- Produces:
  - `#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)] #[serde(default)] pub struct Settings { pub api_base_url: String, pub model: String, pub whisper_model: String, pub language: String, pub notes_dir: Option<PathBuf>, pub name_variants: Vec<String>, pub keywords: Vec<String>, pub general_questions: bool, pub name_bias: bool, pub hotkey: String, pub screenshot_mode: ScreenshotMode, pub monitor: Option<String>, pub region: Option<Region>, pub audio_device: Option<String> }`
  - `Default`: base URL / model / whisper / language per Global Constraints; `keywords = ["homework","assignment","exam","quiz","deadline","attendance"]`; `general_questions = true`; `name_bias = true`; `hotkey = "CommandOrControl+Shift+Space"`; `screenshot_mode = Off`.
  - `pub enum ScreenshotMode { Off, Region, Full }`; `pub struct Region { pub x: i32, pub y: i32, pub width: u32, pub height: u32 }`
  - `pub fn load(path: &Path) -> Settings` — missing → default; unparsable → rename to `settings.json.bak`, return default.
  - `pub fn save(path: &Path, s: &Settings) -> std::io::Result<()>` — temp + rename.
  - `pub fn get_api_key() -> Result<Option<String>, KeyError>`, `pub fn set_api_key(key: &str) -> Result<(), KeyError>` (empty string deletes).

- [ ] **Step 1: Failing tests:** `defaults_match_spec`; `missing_file_gives_default`; `corrupt_file_backed_up_and_default` (write `{not json`, load → default, `.bak` exists with original bytes); `unknown_and_missing_fields_tolerated` (`{"model":"x","future":1}` → model "x", rest default); `save_load_roundtrip`; `api_key_never_serialized` (serialize default Settings → JSON contains no `key`/`token` substring).
- [ ] **Step 2–4:** fail → implement → pass (keychain functions are not unit-tested; they are exercised manually in Task 11).
- [ ] **Step 5:** Commit `feat(core): settings`.

### Task 5: `llm`, `assist`, `summarize`

**Files:**
- Create: `core/src/llm.rs`, `core/src/assist.rs`, `core/src/summarize.rs`
- Test: inline; HTTP mocked with a `tokio::net::TcpListener` serving canned responses on 127.0.0.1 (no mock crate).

**Interfaces:**
- Consumes: `vault::SummaryBody` (Task 3).
- Produces:
  - `pub struct LlmImage { pub media_type: String, pub base64: String }`
  - `pub enum LlmError { Network, Http(u16), BadResponse, MissingKey }` — `Display` never contains the key or the response body.
  - `pub struct LlmClient` — `pub fn new(base_url: &str, api_key: String, model: &str) -> Self` (reqwest, rustls, timeout 60 s); `pub async fn complete(&self, system: &str, user_text: &str, image: Option<&LlmImage>, max_tokens: u32) -> Result<String, LlmError>` — POST `{base}/v1/messages` (trailing slash on base tolerated), headers `x-api-key`, `anthropic-version: 2023-06-01`; user content = optional image block then text block; returns concatenated `text` blocks; one retry on network error or 5xx, none on 4xx.
  - `pub enum Confidence { Low, Medium, High }`; `pub struct Analysis { pub question: String, pub suggested_answer: String, pub confidence: Option<Confidence>, pub raw: Option<String> }` (Serialize, TS)
  - `pub fn assist_prompt(course: &str, recent: &str, has_image: bool) -> (String, String)` — system prompt says: student in a live English lecture, the student was just addressed; identify the question asked of them and give a concise answer they can say aloud; reply ONLY JSON `{"question","suggested_answer","confidence":"low|medium|high"}`.
  - `pub fn parse_analysis(reply: &str) -> Analysis` — strips ``` fences, parses first `{…}`; on failure `question = ""`, `raw = Some(reply)`.
  - `pub fn classify_prompt(course: &str, recent: &str) -> (String, String)` and `pub fn parse_classify(reply: &str) -> bool` — answer `YES`/`NO`; anything else → false.
  - `pub fn chunk_words(text: &str, max_words: usize) -> Vec<String>`
  - `pub async fn summarize(client: &LlmClient, course: &str, transcript: &str) -> Result<SummaryBody, LlmError>` — `max_words = 12_000`; one chunk → single JSON call `{"summary","notes","action_items":[...]}`; several → per-chunk "notes" calls then one reduce call producing the JSON; unparsable final JSON → `SummaryBody { summary: "", notes: raw, action_items: vec![] }`.

- [ ] **Step 1: Failing tests:** `complete_sends_headers_and_parses_text` (mock asserts `x-api-key` and `anthropic-version` present, path `/v1/messages`); `retries_once_on_500_then_ok`; `no_retry_on_401` (mock counts 1 request; error `Http(401)`); `error_display_hides_key` (format error, assert key string absent); `image_block_sent_first`; `parse_analysis_plain_fenced_and_garbage`; `parse_classify_variants` (`"YES"`, `"yes."`, `"No"`, `"maybe"`); `chunk_words_exact_boundaries` (25 words, max 10 → 3 chunks of 10,10,5); `summarize_chunks_long_transcript` (30 000 words → mock sees 3 map calls + 1 reduce call, result parsed).
- [ ] **Step 2–4:** fail → implement → pass.
- [ ] **Step 5:** Commit `feat(core): llm client, assist and summarize`.

### Task 6: STT spike — whisper-rs builds with CUDA and Metal

Risk-first: proves the native build before the pipeline depends on it.

**Files:**
- Create: `core/src/stt/mod.rs`, `core/src/stt/whisper.rs`, `core/src/stt/model.rs`
- Create: `tests/fixtures/jordan.wav` (16 kHz mono, ≤10 s, generated by `scripts/make-fixture.ps1` with Windows SAPI: "Good morning everyone. Today we cover sorting. Jordan, what is the complexity of merge sort?") and `scripts/make-fixture.ps1`
- Modify: `core/Cargo.toml` features `cuda = ["whisper-rs/cuda"]`, `metal = ["whisper-rs/metal"]`

**Interfaces:**
- Produces:
  - `pub enum Backend { Gpu, Cpu }` (Serialize, TS)
  - `pub trait Transcriber: Send { fn transcribe(&mut self, pcm16k: &[f32], prompt: Option<&str>) -> Result<String, SttError>; fn backend(&self) -> Backend; }`
  - `pub struct WhisperTranscriber` — `pub fn load(model_path: &Path, language: &str) -> Result<Self, SttError>` (`use_gpu(true)`; backend reported from whisper system info; `"auto"` → no language set).
  - `pub fn model_file_name(model: &str) -> String` → `ggml-{model}.bin`
  - `pub async fn download_model(model: &str, dir: &Path, progress: impl Fn(u64, u64)) -> Result<PathBuf, SttError>` — URL `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{model}.bin`; expected SHA-256 = `lfs.oid` for that path from `https://huggingface.co/api/models/ggerganov/whisper.cpp/tree/main`; download to `.part`, verify, rename; mismatch → delete `.part`, error.

- [ ] **Step 1:** Generate fixture; commit script + wav.
- [ ] **Step 2:** Test `#[ignore] whisper_tiny_transcribes_fixture` (env `LMA_TEST_MODEL_DIR`; loads `tiny`, transcribes fixture, asserts normalised text contains "merge sort" and "jordan" or "jordon").
- [ ] **Step 3:** Implement; build with `cargo test -p lma_core --features cuda -- --ignored whisper_tiny` after downloading tiny (75 MB) to `%LOCALAPPDATA%\lma-test-models`. Expected: PASS and backend `Gpu` printed.
- [ ] **Step 4:** If CUDA build fails on sm_120/CUDA 12.8, stop and report the exact error to the manager (do not switch backend silently).
- [ ] **Step 5:** Commit `feat(core): whisper transcriber and model download`.

### Task 7: Segmenter, VAD gate and hallucination filter

**Files:**
- Create: `core/src/stt/segmenter.rs`, `core/src/stt/vad.rs`, `core/src/stt/filter.rs`

**Interfaces:**
- Consumes: `Transcriber` (Task 6).
- Produces:
  - `pub trait Vad: Send { fn is_speech(&mut self, frame: &[f32]) -> bool; }` — frame = 480 samples (30 ms @16 kHz); `pub struct EarshotVad` implements it.
  - `pub enum SegEvent { Fast(Vec<f32>), Final(Vec<f32>) }`
  - `pub struct Segmenter` — `pub fn new() -> Self`; `pub fn push(&mut self, samples: &[f32], vad: &mut dyn Vad) -> Vec<SegEvent>`. Rules (Global Constraints): emit `Fast(last ≤8 s)` every 2 s of audio only if that window has ≥300 ms speech; emit `Final(segment)` when ≥600 ms silence follows speech or segment reaches 15 s; segments with <300 ms speech are dropped; ring buffer capped at 30 s.
  - `pub fn is_hallucination(text: &str, prompt: Option<&str>) -> bool` — true if normalised text empty, equals normalised prompt, consists only of prompt tokens, or is one of: "thank you", "thanks for watching", "you", "bye", "subtitles by the amara org community".
  - `pub fn initial_prompt(names: &[String], enabled: bool) -> Option<String>` — `None` if disabled or no names; else `"Participants: A, B, C."`.

- [ ] **Step 1: Failing tests** with a fake VAD (`speech when |sample| > 0.1`): `silence_emits_nothing` (60 s zeros → no events); `noise_below_threshold_emits_nothing`; `speech_then_silence_emits_final` (2 s tone 0.5 amp + 1 s zeros → exactly one `Final` of ~2 s); `fast_every_2s_during_speech` (10 s tone → 5 `Fast`); `final_capped_at_15s` (20 s continuous tone → a `Final` at 15 s); `hallucination_filter_cases` (prompt echo "Participants: Jordan." vs text "Jordan." → true; "Jordan, what is merge sort" → false; "Thank you." → true).
- [ ] **Step 2–4:** fail → implement → pass.
- [ ] **Step 5:** Commit `feat(core): segmenter with VAD gate`.

### Task 8: Audio capture and screenshots

**Files:**
- Create: `core/src/capture/mod.rs`, `core/src/capture/resample.rs`, `core/src/capture/wav.rs`, `core/src/capture/windows.rs` (`#[cfg(windows)]`), `core/src/capture/macos.rs` (`#[cfg(target_os = "macos")]`)
- Create: `core/src/screenshot.rs`

**Interfaces:**
- Produces:
  - `pub trait AudioSource: Send { fn start(&mut self, tx: std::sync::mpsc::Sender<Vec<f32>>) -> Result<(), CaptureError>; fn stop(&mut self); }` — sends 16 kHz mono f32 chunks.
  - `pub fn to_16k_mono(interleaved: &[f32], channels: u16, rate: u32) -> Vec<f32>` — downmix average; resample by linear interpolation (anti-alias by averaging when decimating integer factors 2/3).
  - `pub struct WavSource::new(path: PathBuf, realtime: bool)` (hound).
  - `pub fn system_source(device: Option<&str>) -> Result<Box<dyn AudioSource>, CaptureError>` — Windows: cpal WASAPI loopback on default (or named) output device, polls default-device change every 2 s and restarts the stream; macOS: ScreenCaptureKit stream with audio enabled, video minimal (2×2, 1 fps), returns `CaptureError::PermissionDenied` when screen-recording permission is missing.
  - `pub fn list_output_devices() -> Vec<String>`
  - `pub fn list_monitors() -> Result<Vec<String>, ShotError>`; `pub fn capture(mode: ScreenshotMode, monitor: Option<&str>, region: Option<Region>) -> Result<Option<LlmImage>, ShotError>` — `Off` → `Ok(None)`; crop to region (clamped to monitor bounds; empty after clamp → error); downscale to max width 1600; JPEG quality 80; base64; nothing written to disk.

- [ ] **Step 1: Failing tests:** `resample_48k_stereo_to_16k_length` (48 000 stereo frames → 16 000 samples ±1); `resample_preserves_1khz_tone_energy` (RMS within 10 %); `wav_source_delivers_all_samples` (fixture, realtime=false, sum of chunk lengths = file length); `region_clamped_to_monitor` (pure helper `clamp_region(region, w, h) -> Option<Region>`; off-screen → None).
- [ ] **Step 2–4:** fail → implement → pass on Windows; macOS code must compile in CI (Task 1 workflow).
- [ ] **Step 5:** Manual check (manager runs): play a YouTube lecture, run `cargo run -p lma_core --example loopback_level` (prints RMS once per second) → non-zero RMS. Commit `feat(core): audio capture and screenshots`.

### Task 9: Session orchestrator

**Files:**
- Create: `core/src/session.rs`; Test: `core/tests/session.rs`

**Interfaces:**
- Consumes: Tasks 2–8.
- Produces:
  - `#[derive(Serialize, TS, Clone, Debug)] #[serde(tag = "type")] pub enum SessionEvent { Partial { text: String }, Line { time: String, text: String }, Hit { kind: HitKind, term: String, context: String }, AnalysisStarted { id: u64, reason: String }, Analysis { id: u64, time: String, analysis: Analysis }, AnalysisFailed { id: u64, error: String }, QuestionDetected { text: String }, Level { rms: f32 }, NoAudio, Backend { backend: Backend }, SummaryWritten { path: String }, SummaryFailed { error: String }, Error { message: String } }`
  - `pub struct SessionConfig { pub course: String, pub files: SessionFiles, pub date: NaiveDate, pub names: Vec<String>, pub keywords: Vec<String>, pub general_questions: bool, pub name_bias: bool, pub screenshot: (ScreenshotMode, Option<String>, Option<Region>) }`
  - `pub type Sink = Arc<dyn Fn(SessionEvent) + Send + Sync>`
  - `pub struct Session` — `pub fn start(cfg: SessionConfig, source: Box<dyn AudioSource>, stt: Box<dyn Transcriber>, vad: Box<dyn Vad>, llm: Option<Arc<LlmClient>>, rt: tokio::runtime::Handle, sink: Sink) -> Result<Session, SessionError>`; `pub fn ask_now(&self)`; `pub fn stop(self) -> Result<(), SessionError>` (stops audio, waits for in-flight final pass, flushes transcript, spawns summary; summary result arrives as `SummaryWritten`/`SummaryFailed`); `pub async fn retry_summary(files: &SessionFiles, course: &str, date: NaiveDate, llm: &LlmClient) -> Result<(), SessionError>` (reads transcript body after frontmatter).
  - Behaviour: STT runs on a dedicated thread; fast-pass text → `Partial` + `Matcher::scan` → `Hit`; name hit → `AnalysisStarted` + spawned assist (screenshot captured at hit time); final text (after `is_hallucination` filter) → `TranscriptWriter::append` + `Line`; general question → `RateLimiter` then classify → `QuestionDetected` + assist; `Level` every 1 s; `NoAudio` once after 10 s with RMS < 0.001 (re-armed when audio returns); llm `None` → assist emits `AnalysisFailed { error: "API key missing" }`.
  - Append failure: line kept in an in-memory pending queue, `Error` emitted once per failure streak, queue retried on the next line and on stop.

- [ ] **Step 1: Failing tests** using `WavSource` + a `FakeTranscriber` (returns scripted strings) + fake VAD + mock LLM from Task 5:
  - `name_in_fast_pass_emits_hit_then_analysis`
  - `silence_wav_produces_no_lines_or_hits`
  - `transcript_file_gets_final_lines`
  - `append_failure_is_buffered_and_retried` (make transcript file read-only mid-run, then writable; final file contains all lines in order)
  - `no_audio_event_after_10s_of_silence` (use non-realtime source and injected clock or 10 s zeros)
  - `missing_llm_reports_api_key_missing`
  - `#[ignore] end_to_end_tiny_model` (real `WhisperTranscriber` tiny + `jordan.wav` + names `["Jordan"]` → a `Hit` and ≥1 `Line`).
- [ ] **Step 2–4:** fail → implement → pass.
- [ ] **Step 5:** Commit `feat(core): session orchestrator`.

### Task 10: Tauri app wiring

**Files:**
- Modify: `src-tauri/src/lib.rs`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`
- Create: `src-tauri/src/state.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/notify.rs`

**Interfaces:**
- Consumes: `lma_core::*`.
- Produces (Tauri commands; argument/return types exported with ts-rs to `ui/src/bindings/`):
  - `get_settings() -> Settings`, `save_settings(settings: Settings) -> Result<(), String>` (re-registers hotkey)
  - `has_api_key() -> bool`, `set_api_key(key: String) -> Result<(), String>`
  - `list_courses() -> Result<Vec<String>, String>`
  - `start_session(course: String) -> Result<(), String>`, `stop_session() -> Result<(), String>`, `ask_now() -> Result<(), String>`, `retry_summary() -> Result<(), String>`
  - `model_status() -> ModelStatus { installed: bool, model: String }`, `download_model() -> Result<(), String>` (progress via event `model-progress { done, total }`)
  - `list_monitors() -> Result<Vec<String>, String>`, `list_output_devices() -> Vec<String>`
  - `open_region_picker() -> Result<(), String>` (opens transparent fullscreen window `region` loading `ui` route `#/region`; that window calls `save_region(region: Region)`)
  - Event `session-event` with `SessionEvent` payload.
  - `AppState { settings: Mutex<Settings>, session: Mutex<Option<Session>>, last_files: Mutex<Option<(SessionFiles, String, NaiveDate)>> }`
  - Notifications (`tauri-plugin-notification`) from the sink: `Hit::Name` → title "Your name was mentioned", body = last 120 chars of context; `Hit::Keyword` → "Keyword: <term>"; `Analysis` → title "Question", body = question + " → " + answer (truncated 200); `QuestionDetected` → "Question to the class" (silent).
  - Global shortcut from `settings.hotkey` → same as `ask_now`; invalid hotkey string → `save_settings` returns error, old hotkey kept.
  - Window close → hide; tray menu Show / Start-Stop / Quit.
- [ ] **Step 1: Failing tests** (pure state logic extracted to `state.rs`, no Tauri runtime): `second_start_rejected`, `stop_without_session_errors`, `ask_now_without_session_errors`, `start_requires_notes_dir_and_model` (clear error strings: "Set the notes folder in Settings first", "Download the Whisper model first").
- [ ] **Step 2–4:** fail → implement → pass; `cargo tauri dev` launches.
- [ ] **Step 5:** Commit `feat(app): tauri commands, notifications, hotkey, tray`.

### Task 11: UI

**Files:**
- Create: `ui/src/App.tsx` (hash routes `#/`, `#/settings`, `#/region`), `ui/src/views/Main.tsx`, `ui/src/views/Settings.tsx`, `ui/src/views/RegionPicker.tsx`, `ui/src/components/CoursePicker.tsx`, `ui/src/api.ts` (typed `invoke` wrappers using generated bindings), tests `*.test.tsx`

**Interfaces:**
- Consumes: commands/events from Task 10 via `ui/src/bindings/`.
- Behaviour:
  - Main: Start opens CoursePicker (list + "New course…" text field); Stop; Ask now; live transcript (last 20 `Line`s, latest `Partial` in grey); analysis cards newest first (time, question, suggested answer, confidence; `raw` shown when question empty; `AnalysisFailed` as error card); status bar: backend (`CPU mode — slow` warning), level meter, "No audio detected — check the output device", last error; `SummaryFailed` shows **Retry summary**.
  - Settings: all `Settings` fields; API key field is write-only (shows "Saved" if `has_api_key`); text under API fields: "Transcript excerpts, the full transcript for summaries, and screenshots (if enabled) are sent to this provider. Audio never leaves this computer."; name variants and keywords as one-per-line textareas; model Download with progress bar; notes folder via dialog plugin folder picker.
  - RegionPicker: full-window drag rectangle, Enter/Save calls `save_region`, Esc closes.
  - All user/transcript text rendered as JSX text only.
- [ ] **Step 1: Failing tests** (Vitest + Testing Library, `invoke` mocked): `start_opens_course_picker_and_calls_start_session`; `analysis_event_renders_card_newest_first`; `api_key_input_not_prefilled`; `settings_save_sends_trimmed_nonempty_variants`; `transcript_text_with_html_is_escaped` (line `<img src=x onerror=alert(1)>` renders as text, no `img` element).
- [ ] **Step 2–4:** fail → implement → pass; `pwsh scripts/check.ps1` → `CHECK PASSED`.
- [ ] **Step 5:** Commit `feat(ui): main, settings and region picker`.

### Task 12: README, security review, release build

**Files:**
- Create: `README.md` (what it does, privacy section: audio local / text to provider, setup on Windows (CUDA 12.x) and macOS (Screen Recording permission), `cargo tauri build --features lma_core/cuda`, settings guide, vault layout)
- Create: `docs/security-review-2026-10-22.md` (reviewer output)

- [ ] **Step 1:** Separate read-only security reviewer worker reviews the whole tree against STANDARDS.md: path traversal, frontmatter injection, key leakage (logs, events, errors, settings JSON), panics, data sent to the network, repo hygiene (no personal data). Report with line numbers.
- [ ] **Step 2:** A different worker fixes findings; gate → `CHECK PASSED`.
- [ ] **Step 3:** `cargo tauri build --features lma_core/cuda` → `.msi` produced; manual smoke test by manager + user in a real Teams lecture (name trigger latency ≤3 s, summary file appears in the vault).
- [ ] **Step 4:** Manager runs `git grep -n -i -E` with the user's name, OS username, vault path and key prefixes (patterns kept outside the repo) → no hits. Commit `docs: readme and security review`; push.
- [ ] **Step 5:** User pulls on MacBook; `cargo tauri dev --features lma_core/metal`; verify audio capture permission flow, notification, hotkey, summary write.
