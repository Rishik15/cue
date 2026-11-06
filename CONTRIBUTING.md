# Contributing

Bug reports and ideas are welcome. Open an [issue](https://github.com/Rishik15/cue/issues/new/choose) and say what you did, what you expected and what happened. Your Windows version and Cue's version (on the About page) help. For anything security related, use the private form described in [SECURITY.md](SECURITY.md) instead.

## Building it yourself

You need Rust (MSVC), Node.js 24 or newer, pnpm and the Visual Studio C++ Build Tools. The [README](README.md#build-from-source) has the first commands. Everything goes through one script, `scripts/cue.ps1` (run `./scripts/cue.ps1 help` for all modes):

- `./scripts/cue.ps1 check`: type check, ESLint with no warnings, the file size limit, the theme contrast check, `cargo fmt --check` and `cargo clippy -D warnings`. Add `-Deps` for the unused-crate and advisory checks (needs `cargo install cargo-machete cargo-deny`).
- `./scripts/cue.ps1 test`: the Rust tests.
- `./scripts/cue.ps1 release`, then `bench`, `smoke` or `threads`: the optimized exe and the end-to-end checks CI also runs.

Working from WSL instead? `scripts/wsl/winsync <mode>` copies the repo to a Windows folder and runs the same modes there. Copy `scripts/wsl/winsync.env.example` to `scripts/wsl/winsync.env` to change the folder or tool paths.

The code follows a few rules: short functions and files, every call into Windows inside `src-tauri/src/platform/`, no dependency for something a few lines can do, and untrusted text checked where it comes in.
