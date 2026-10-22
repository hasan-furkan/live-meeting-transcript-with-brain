# Security review — 2026-10-22

Scope: `core/src`, `core/build.rs`, `src-tauri/src`, `src-tauri/build.rs`,
`src-tauri/tauri.conf.json`, `src-tauri/capabilities/`, `ui/src` (excluding
`ui/src/bindings`), `scripts/`, `.github/`, `.gitignore`. Read-only review; no
source file was changed. Requirements checked against `STANDARDS.md` and spec
sections 2, 6, 7 and 9.

Severity counts: **Critical 0 · Important 2 · Minor 9**.

---

## Critical

None.

No path escape from the notes folder, no secret in logs/events/errors, no audio
or screenshot written to disk, and no crash path found in non-test code.

---

## Important

### I1. `save_settings` accepts any API base URL; the backend will send the key and lecture text there

- `src-tauri/src/commands.rs:49` — `save_settings` stores the webview-supplied
  `Settings` verbatim (no URL parsing or scheme check).
- `core/src/llm.rs:64` — the endpoint is built by string concatenation:
  `format!("{}/v1/messages", base_url.trim_end_matches('/'))`.
- `core/src/llm.rs:129-133` — the key is sent as `x-api-key` to that endpoint on
  every analysis, classification and summary.

Issue: any scheme and host is accepted, including `http://` and arbitrary
domains. A compromised or malicious webview (XSS, or a supply-chain injection in
the bundled UI) cannot read the keychain directly, but it can call
`save_settings` with `api_base_url` pointing at an attacker host, then
`ask_now` / `retry_summary` to make the Rust backend send the API key and
lecture text there — over plaintext if `http://` is used. A user who types an
`http://` URL gets the same plaintext exposure.

Impact: API-key and lecture-content exfiltration outside the configured
provider, and cleartext transmission of both.

Suggested fix: validate inside the backend (not the UI): parse with `url::Url`,
require `https` (optionally allow `http://localhost`/`127.0.0.1` for local
proxies), reject userinfo and non-empty query/fragment, and return a static
"Invalid API URL" error. Re-validate in `LlmClient::new` as the last line of
defence.

### I2. `save_settings` accepts an arbitrary `notes_dir` and model name from the webview

- `src-tauri/src/commands.rs:49-66` — no validation of `notes_dir`,
  `whisper_model`, `language` or list lengths.
- `src-tauri/src/commands.rs:79-86` — `list_courses` lists the immediate
  subdirectories of whatever `notes_dir` holds.
- `src-tauri/src/commands.rs:105-121` — `start_session` joins the model name into
  `models_dir` and calls `allocate_session(&notes, course, date)`, which creates
  the course folder and two markdown files under `notes_dir`.

Issue: the backend trusts webview input for filesystem locations. A compromised
webview can set `notes_dir` to any directory, enumerate its immediate
subdirectory names through `list_courses`, and create course folders plus
`*-transcript.md` / `*-ders.md` files anywhere the user can write (contents are
transcript/summary data, not attacker-chosen). `whisper_model` is likewise
unvalidated (see M1).

Impact: directory-name disclosure and markdown-file creation outside the chosen
notes folder, if the webview is compromised. Path traversal out of the notes
folder through the course name itself is correctly blocked (`core/src/vault.rs:43-129`),
so this is about the base folder, not the course segment.

Suggested fix: treat `notes_dir` as a capability granted only by the folder
dialog — e.g. an `AppState` field set by a `choose_notes_dir` command that uses
the dialog plugin and canonicalises the path, rather than a field writable via
`save_settings`; at minimum canonicalise it, require it to exist and be a
directory, and reject model names outside `[A-Za-z0-9._-]`.

---

## Minor

### M1. Whisper model name is joined into a path without validation

`src-tauri/src/commands.rs:107` (`state.models_dir.join(model_file_name(...))`),
`src-tauri/src/commands.rs:228`, `src-tauri/src/commands.rs:235`;
`core/src/stt/model.rs:10-12,35-36`.

`model_file_name` only prefixes `ggml-` and appends `.bin`, so `..` and path
separators survive. Exploitation is limited: the first component is always
`ggml-…`, which normally does not exist, so the OS cannot resolve `..`; and the
download path looks the name up in the Hugging Face tree before creating the
file (`core/src/stt/model.rs:51,64`), so a non-listed name fails first. Impact is
a confusing failure or, with a pre-created `models/ggml-..` directory, a
traversal. Fix: validate the model name in the backend before use.

### M2. An already-present model file is trusted without a checksum

`core/src/stt/model.rs:37-39` returns `dest` as soon as `is_file()`.

The SHA-256 is verified only on download (`core/src/stt/model.rs:80-84`).
A corrupted, truncated or planted `ggml-*.bin` in the app-data folder is loaded
by whisper.cpp (C code) unchecked. Requires write access to app data, so low
impact, but it contradicts "checksum verified before use". Fix: persist the
expected hash (or re-fetch the tree entry) and verify on load, or at least
verify size, and offer re-download on mismatch as the spec already does.

### M3. Unbounded in-memory transcript and pending queue

`core/src/session.rs:381` (`lines: Vec<String>`), `core/src/session.rs:485`
(every final line pushed), `core/src/session.rs:572-579` (`recent_text` joins the
whole vector on every analysis and name hit), `core/src/session.rs:377,595`
(`pending` grows while disk writes fail).

The 30 s audio ring is correctly capped (`core/src/stt/segmenter.rs:14,80-82`),
but the transcript is kept for the whole session: a multi-hour lecture grows it
without bound, and each analysis copies it again (`join`). If the notes folder
becomes unwritable mid-session, every line is queued for retry indefinitely.
Fix: keep only the last N words for prompts (a rolling window is all the
triggers need) and write the summary from the transcript file, which
`retry_summary` already does (`core/src/session.rs:314-316`); cap `pending` at a
few hundred lines.

### M4. Summary write has a check-then-rename race

`core/src/vault.rs:191-196` checks `files.summary.exists()`, then
`core/src/vault.rs:222-229` writes a `.tmp` and renames it.

Between the check and the rename another process could create the summary path;
on Unix `rename` would then replace it. The transcript is safe (`create_new`,
`core/src/vault.rs:152`). A stale `.tmp` from a crash is truncated on the next
run, which is fine. Fix: create the final name with `OpenOptions::create_new`
(and write in place) or re-check immediately before rename and treat
`AlreadyExists` as the existing error.

### M5. Transcript appends follow symlinks and re-open the path each line

`core/src/vault.rs:168-175`.

`append` intentionally re-opens by path per line. A local process with write
access to the notes folder could swap the file for a symlink between lines and
redirect lecture text elsewhere; the same class applies to a symlinked summary
path. Low risk (local write access to the user's notes), but it is the one place
where the "only inside the notes folder" guarantee depends on the folder's
integrity at write time. Fix (optional): keep a single handle opened with
`create_new` semantics for the transcript, or canonicalise the path and compare
against the session folder before each append.

### M6. Model download client has no timeout and follows redirects blindly

`core/src/stt/model.rs:42` (`reqwest::Client::new()`), `core/src/stt/model.rs:53-57`.

There is no request timeout, so a stalled connection can hang the download
command forever with no cancel path. The default redirect policy can also follow
an HTTPS→HTTP redirect. Both endpoints are HTTPS constants
(`core/src/stt/model.rs:7-8`) and the hash is still verified, so impact is
availability/downgrade hardening only. Fix: add `.timeout(...)` and a redirect
policy that rejects non-HTTPS targets.

### M7. CSP can be tightened

`src-tauri/tauri.conf.json:25`:
`default-src 'self' ipc: http://ipc.localhost; img-src 'self' data:; style-src 'self' 'unsafe-inline'`.

No remote scripts are allowed (`default-src` covers `script-src`), so this is
sound. Missing hardening directives: `object-src 'none'`, `base-uri 'none'`
(not covered by `default-src`), `frame-ancestors 'none'`, and an explicit
`connect-src ipc: http://ipc.localhost`. `style-src 'unsafe-inline'` is needed
for the region overlay's React inline styles. `img-src data:` is currently
unused and could be dropped. Fix: extend the CSP as above when the UI is next
touched.

### M8. macOS capture start can block forever

`core/src/capture/macos.rs:38` waits on `ready_rx.recv()` with no timeout; the
thread only replies after `SCShareableContent::get()` and `start_capture()`
return (`core/src/capture/macos.rs:55-65`). The Windows source uses a 5 s
timeout (`core/src/capture/windows.rs:41`). If ScreenCaptureKit stalls,
`start_session` never returns and the session gate stays open. Availability, not
confidentiality. Fix: mirror the Windows `recv_timeout` + stop-flag pattern.

### M9. CI actions are pinned to tags, not commit SHAs

`.github/workflows/ci.yml:11-20` (`actions/checkout@v4`,
`dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`,
`taiki-e/install-action@v2`, `actions/setup-node@v4`).

A moved tag or compromised action could run arbitrary code in the gate, which
has the repository token and (on a release workflow) signing secrets. Fix: pin
each action to a full commit SHA with the version in a comment.

---

## Verified OK

- **Path traversal / output location.** Course names drop `/ \ : * ? " < > |`,
  turn control characters into spaces, trim dots/spaces, reject empty/`..`
  (`core/src/vault.rs:43-60`); the notes folder and course folder are
  canonicalised and `starts_with` is enforced (`core/src/vault.rs:99-108`);
  the date stem is generated, not user input (`core/src/vault.rs:109-123`).
- **No overwrites.** Transcript uses `create_new` (`core/src/vault.rs:152`);
  a session suffix is allocated when either file exists
  (`core/src/vault.rs:110-123`); summary refuses an existing file
  (`core/src/vault.rs:191-196`).
- **Frontmatter injection.** The course value is JSON-encoded (a valid YAML
  double-quoted scalar) and `visibility`/`tur` are constants
  (`core/src/vault.rs:131-138`); transcript text is flattened to one line
  (`core/src/vault.rs:140-144,170`); tests cover a malicious course name
  (`core/src/vault.rs:286-300`). A transcript line cannot open or close the
  frontmatter block because it is written after the block and has no privileged
  position; control characters are stripped.
- **API key storage.** Only in the OS keychain (`core/src/settings.rs:126-151`);
  `Settings` has no key field and serialisation contains no key (`core/src/settings.rs:28-43,252-258`);
  the UI field is a write-only password input and is never prefilled
  (`ui/src/views/Settings.tsx:166-170`).
- **Secrets in logs / events / errors.** Only two `eprintln!` calls, both static
  messages (`src-tauri/src/commands.rs:59`, `src-tauri/src/lib.rs:117`);
  `LlmError` strings are static and hide status bodies and key
  (`core/src/llm.rs:17-34,137-141`; privacy test
  `core/src/llm.rs:279-288`); `SessionEvent` variants carry transcript/analysis
  text, never the key (`core/src/session.rs:40-91`); notifications use the same
  sanitised errors (`src-tauri/src/state.rs:157-197`).
- **HTTP response bodies never surfaced.** Errors map to status codes only
  (`core/src/llm.rs:126-141`); model download errors are static
  (`core/src/stt/mod.rs:17-35`).
- **Network destinations.** The only `reqwest` calls are the configured LLM
  endpoint (`core/src/llm.rs:64,127-134`) and the two Hugging Face HTTPS
  constants (`core/src/stt/model.rs:7-8`). No other network code in `core/src`
  or `src-tauri/src`; the test HTTP server is `#[cfg(test)]`
  (`core/src/lib.rs:16-17`). No listening socket.
- **Audio never written.** Capture produces `Vec<f32>` over an `mpsc` channel
  (`core/src/capture/mod.rs:21-24`); `WavWriter` is test-only
  (`core/src/capture/wav.rs:79-98`); the ring buffer is capped at 30 s
  (`core/src/stt/segmenter.rs:14,80-82`). No audio bytes reach the LLM
  (`core/src/llm.rs:98-123` sends text + optional image only).
- **Screenshots never written; Off sends nothing.** Capture is in memory and
  base64-encoded (`core/src/screenshot.rs:1,93-106`), no filesystem calls in
  that module; `Off` returns before spawning capture and before any request
  (`core/src/session.rs:681-683`, `core/src/screenshot.rs:65-67`), and the
  analysis request only includes an image when one was produced
  (`core/src/session.rs:658-661`).
- **No panics in non-test code.** No `unwrap`/`expect`/`panic!` reachable outside
  tests; the apparent ones are fallible-by-design fallbacks
  (`core/src/llm.rs:61`, `core/src/triggers.rs:32,146-149`,
  `core/src/screenshot.rs:71`, `core/src/vault.rs:133,219`,
  `src-tauri/src/state.rs:73-77`, poisoned locks). Indexing and slicing were
  checked: `core/src/triggers.rs:153-160` and `src-tauri/src/state.rs:205-221`
  cut by chars, `core/src/llm.rs:160-164` slices only at ASCII brace bytes,
  `core/src/session.rs:695-698` clamps the start index, and
  `core/src/capture/resample.rs:39-42`, `core/src/capture/macos.rs:141-147`
  clamp indices. The clippy gate denies `unwrap_used`/`expect_used`/`panic`
  (`scripts/check.ps1:23`).
- **Tauri surface.** Capabilities are limited to `core:default`, notification,
  global-shortcut register/unregister, dialog open and window close
  (`src-tauri/capabilities/default.json`); no filesystem plugin and no command
  that reads an arbitrary path — `retry_summary` reads only the transcript path
  remembered at start (`src-tauri/src/commands.rs:202-211`), `list_courses`
  only lists the configured folder, `model_status` only stats one file. CSP is
  set (`src-tauri/tauri.conf.json:24-26`); `GlobalTauri` is not enabled and the
  HTML has no inline script (`ui/index.html:10`). IPC input issues are I1/I2/M1.
- **UI injection.** No `dangerouslySetInnerHTML`, `eval`, `new Function` or
  `innerHTML` in `ui/src`; transcript, partial, analysis and error strings are
  rendered as JSX text (`ui/src/views/Main.tsx:153,157,168-177,190-196`), with a
  regression test for HTML in a transcript line
  (`ui/src/views/Main.test.tsx:66-69`).
- **Model download integrity.** HTTPS only, SHA-256 and size taken from the HF
  LFS entry, hashed while streaming, `.part` removed on mismatch and renamed
  only after the hash matches (`core/src/stt/model.rs:42-84`).
- **Repo hygiene.** `git ls-files` plus content search for personal names,
  account names, home paths and key-like strings found nothing outside the
  fictional fixture (only an unrelated `sha512-…` substring in
  `ui/package-lock.json`). `.gitignore` covers build output, `node_modules`,
  `*.bin`/models, `settings.json`, `*.log`, `*.key`/`*.pem`/`.env`, and audio
  except the synthetic fixture (`.gitignore`); tracked WAV is the synthetic
  "Jordan" fixture generated by `scripts/make-fixture.ps1`.

---

## Resolution

Fixed in commit `fix: validate settings input, harden downloads (security review)`
unless noted otherwise. Findings not listed here are unchanged.

### Important

- **I1 — Fixed.** `validate_api_base_url` (`core/src/llm.rs`) parses with
  `reqwest::Url`, requires `https` (plain `http` only for `localhost`,
  `127.0.0.1` and `[::1]`), and rejects userinfo, query and fragment.
  `save_settings` returns `Invalid API URL`; `start_session` starts without
  assist (same as a missing key) and `retry_summary` returns the error when the
  stored URL is invalid. Tests cover the accepted and rejected forms.
- **I2 — Fixed (minimum scope).** `save_settings` canonicalises a supplied
  `notes_dir`, requires it to exist and be a directory (`Notes folder not
  found`) and stores the canonical path; the Whisper model name is validated
  (M1). Deferred: making `notes_dir` a capability granted only by the folder
  dialog instead of a `save_settings` field — the UI still sends the dialog
  path, but a compromised webview can name any directory it can reach.
  `language`, `model` and list lengths remain unvalidated (no filesystem or
  network target).

### Minor

- **M1 — Fixed.** `valid_model_name` (`core/src/stt/model.rs`: non-empty,
  ≤ 64 chars, `[A-Za-z0-9._-]`, no `..`) is enforced by `save_settings`
  (`Invalid Whisper model name`), `download_model` before any network access,
  and `WhisperTranscriber::load` on the `ggml-<name>.bin` file name.
- **M2 — Deferred.** Verifying an already-present model file needs a persisted
  expected hash; the download itself is still hash-checked.
- **M3 — Deferred.** A rolling prompt window and a capped `pending` queue are a
  session-memory refactor, outside the settings/download input boundary.
- **M4 — Deferred.** Closing the summary check-then-rename race needs a
  `create_new` write path in `vault.rs`; the transcript already uses
  `create_new`.
- **M5 — Deferred.** Keeping one transcript handle open for the whole session
  changes the append design; risk requires local write access to the notes
  folder.
- **M6 — Fixed.** The download client has a 15 s connect timeout, a 60 s read
  timeout per chunk, and a redirect policy capped at 10 hops that only follows
  `https` targets.
- **M7 — Deferred.** CSP hardening (`object-src`, `base-uri`,
  `frame-ancestors`, `connect-src`) belongs with the next `tauri.conf.json`
  change.
- **M8 — Fixed.** macOS capture `start()` waits at most 10 s for the ready
  signal (`recv_timeout`), sets the stop flag and returns
  `CaptureError::Failed`, mirroring Windows.
- **M9 — Deferred.** Pinning GitHub Actions to commit SHAs means editing
  `.github/`, which the implementer rules keep out of worker scope; left to the
  maintainer.
