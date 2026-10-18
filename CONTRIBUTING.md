# Contributing

Cue is not accepting pull requests yet. I'm building the first versions on my own, so the code is public to read, run and learn from, but changes come from me for now. Pull requests are switched off for people outside the project.

Bug reports and ideas are welcome. Open an [issue](https://github.com/Rishik15/cue/issues/new/choose) and say what you did, what you expected and what happened. Your Windows version and Cue's version (on the About page) help. For anything security related, use the private form described in [SECURITY.md](SECURITY.md) instead.

## If you want to build it yourself

You need Rust (MSVC), Node.js 24 or newer, pnpm and the Visual Studio C++ Build Tools. The [README](README.md#build-it) has the commands. The checks I run before every push are:

- `pnpm check`: type check, ESLint with no warnings, the file size limit and the theme contrast check
- `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test` in `src-tauri`

The code follows a few rules: short functions and files, every call into Windows inside `src-tauri/src/platform/`, no dependency for something a few lines can do, and untrusted text checked where it comes in. When contributions open, this page will say so.
