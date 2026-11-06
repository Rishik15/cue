# Contributing

Open an [issue](https://github.com/Rishik15/cue/issues/new/choose) with what you did, what you expected and what happened, plus your Windows and Cue versions (About page). Security problems go through the private form in [SECURITY.md](SECURITY.md).

## Build and check

Requires Rust (MSVC), Node.js 24+, pnpm and the Visual Studio C++ Build Tools. Everything runs through `scripts/cue.ps1` (`./scripts/cue.ps1 help` lists modes):

- `check`: type check, ESLint with no warnings, size limit, theme contrast, `cargo fmt --check`, `cargo clippy -D warnings`. `-Deps` adds `cargo-machete` and `cargo-deny`.
- `test`: Rust tests.
- `release`, then `bench`, `smoke` or `threads`: the optimized build and the end-to-end checks CI runs.

From WSL, `scripts/wsl/winsync <mode>` copies the repo to Windows and runs the same modes; see `scripts/wsl/winsync.env.example`.

## Code rules

Short functions and files, every Windows call inside `src-tauri/src/platform/`, no dependency for what a few lines can do, untrusted text checked where it enters.
