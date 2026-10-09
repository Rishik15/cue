# Contributing to Cue

Thanks for helping. Cue is a small, local-first desktop app, so the bar is simple: keep it fast, quiet at rest, and easy to read.

## Ground rules

- **Be kind.** Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md).
- **Discuss big changes first.** Open an issue before starting anything large (a new feature, a new dependency, a new window). Small fixes can go straight to a pull request.
- **Privacy is a feature.** Text, audio and screenshots are processed on the device. Do not add network calls beyond explicit model downloads and update checks, and never log user content.
- **Idle cost is a feature.** Cue must cost nothing while it sits in the tray: no polling loops, no periodic timers, no resident helper windows. Block on a channel, a message queue or an OS event, and free whatever a feature creates when the feature ends.

## Setup

Prerequisites: Rust (stable, MSVC toolchain on Windows), Node.js 24 or newer, pnpm (`corepack enable`), and on Windows the Visual Studio Build Tools (C++ workload) and the WebView2 runtime.

```sh
pnpm install
pnpm tauri dev      # run the app with hot reload
pnpm tauri build    # produce the installer
```

The tray menu, recording overlay, text insertion and clipboard code are Windows-only for now; they sit behind `src-tauri/src/platform/` so other platforms can add their own implementations without touching callers.

## Before you open a pull request

```sh
pnpm check                                                        # types, ESLint (no warnings), size limits
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

CI runs the same checks. For anything that touches the idle path, also run the benchmark (`pwsh scripts/bench.ps1`, needs a release build) and compare with `bench/baseline.json`.

## Code conventions

- **Size:** functions at most 50 lines, files at most 250 code lines, nesting depth at most 3. Split at a meaningful boundary.
- **Structure:** one responsibility per module, function and component. Group by feature (`shortcut/`, `audio/`). Keep pure logic in its own file so it can be tested without the OS.
- **OS calls** live in `src-tauri/src/platform/`; callers never import `windows::` directly. New capabilities get a Windows implementation and a stub so every target still compiles.
- **Frontend:** state in hooks, presentation in components, shared pieces in `src/ui/`. Pages only assemble kit components and never set raw sizes or colours. Truncate long names with an ellipsis, cap widths, and design empty and error states, not just the filled one.
- **Errors:** no `unwrap` on fallible input. Failures the user can act on go through `status::report` so they are visible, not just logged.
- **Dependencies:** prefer the standard library or what is already installed. A new crate or package needs a reason a few lines of code cannot meet.
- **Comments and headers:** non-trivial files start with a `Purpose:` and `Contents:` header. Comment intent and non-obvious decisions, not what the code already says.
- **Tests:** a unit test for new logic or boundary checks. No mocking frameworks.

## Commit messages

One short line in the imperative that finishes the sentence "This commit will ...", for example `Add settings window` or `Fix theme flash on first paint`. No body.

## Reporting bugs and security issues

Use the issue templates for bugs and ideas. Report vulnerabilities privately as described in [SECURITY.md](SECURITY.md).

## License

By contributing you agree that your contribution is licensed under the [MIT License](LICENSE).
