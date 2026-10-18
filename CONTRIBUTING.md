# Contributing

Thanks for helping with Cue. Bug reports, ideas and pull requests are all welcome.

## Reporting a problem

Open an [issue](https://github.com/Rishik15/cue/issues/new/choose) and say what you did, what you expected and what happened. Your Windows version and Cue's version (About page) help. If the problem is a security one, use the private form described in [SECURITY.md](SECURITY.md) instead.

## Building and checking

You need Rust (MSVC), Node.js 24 or newer, pnpm and the Visual Studio C++ Build Tools. See the [README](README.md#build-it) for the commands. Before you open a pull request, run:

- `pnpm check`: type check, ESLint with no warnings, the file size limit and the theme contrast check
- `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test` in `src-tauri`

CI runs the same checks plus a memory benchmark, so a pull request that raises idle memory or CPU will fail it. Cue is meant to stay at a few MB and 0 % CPU while it waits.

## Code

- Keep functions short (50 lines at most) and files under 250 lines. Split along real boundaries instead of squeezing.
- Everything that talks to Windows lives in `src-tauri/src/platform/`. Other code never imports `windows::`.
- Don't add a dependency if a few lines will do. Don't add settings, traits or options for things that don't vary yet.
- Text from a model, a file or the clipboard is untrusted: check it where it comes in.
- Add a test for each new branch, parser or boundary check.
- UI is built from the components in `src/ui/`. Pages don't set raw sizes or colors, and colors come from `src/themes.json`.

## Pull requests

Keep a pull request to one change. Write each commit as one short line in the imperative that finishes "This commit will ...", for example `Add settings window` or `Fix theme flash on first paint`. Say in the description what you tried on Windows. Anything that touches typing, shortcuts or the clipboard should be tried by hand in a few apps, since unit tests can't cover that.
