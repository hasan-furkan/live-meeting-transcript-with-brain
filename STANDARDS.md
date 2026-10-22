# Project standards

Read this file and the spec (`docs/superpowers/specs/2026-10-22-live-meeting-assistant-design.md`) before any work.

## Stack

- Rust core (`core/`, crate `lma_core`) holds all logic. Tauri 2 app (`src-tauri/`, crate `lma_app`) is thin wiring.
- UI (`ui/`): React + TypeScript (strict) + Vite. The UI only displays and calls Tauri commands; no business logic.
- Command/event types are generated from Rust with `ts-rs` into `ui/src/bindings/`. Never hand-write them.

## Rust rules

- `#![forbid(unsafe_code)]` at every crate root.
- Must pass: `cargo clippy --workspace --all-targets -- -D warnings -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic -D clippy::todo -D clippy::dbg_macro`. `unwrap`/`expect`/`panic` are allowed only in tests (`#[cfg(test)]`, `#[test]`, and `core/tests/*.rs` files that start with `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]`).
- `cargo fmt --all --check`, `cargo test --workspace`, `cargo audit`, `cargo deny check` must pass.
- **No new dependency** (crate or npm package) unless the task brief names it. Need another one? Stop and report it with a reason.
- Error messages never contain file contents, API keys, tokens or HTTP response bodies. Secrets are never logged.
- No network listener. Network calls go only to the configured LLM URL and the Hugging Face model URL.

## Privacy rules (public repository)

- Audio is never written to disk. Screenshots are never written to disk.
- Never commit lecture content, transcripts, notes, settings, keys, tokens, model files, real personal names or personal paths. Test data uses the fictional name "Jordan Reyes" and course "Algorithms 101".
- The API key lives only in the OS keychain.

## React rules

- `tsconfig`: `strict`, `noUncheckedIndexedAccess`, `noImplicitOverride`. No `any` (`@typescript-eslint/no-explicit-any: error`).
- ESLint: typescript-eslint strict + `eslint-plugin-react-hooks`.
- Forbidden: `dangerouslySetInnerHTML`, `eval`, `new Function`. User/transcript text is rendered only as JSX text.
- Tests: Vitest + Testing Library.

## Language

UI text, README, comments, generated notes: English.

## Quality gate

`pwsh scripts/check.ps1` (or `powershell -File scripts/check.ps1`) must end with `CHECK PASSED` before a task is done. Workers must not edit `scripts/check.ps1`, `deny.toml`, `clippy.toml` or this file.

## Commits

Small commits, conventional messages (`feat(core): …`, `test(core): …`, `chore: …`). Commit only on the current branch; never push.
