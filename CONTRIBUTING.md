# Contributing

Open an [issue](https://github.com/Rishik15/cue/issues/new/choose) with what you did, what you expected, what happened, and your Windows and Cue versions (About page). Report security problems through the private form in [SECURITY.md](SECURITY.md).

## Setup

Cue is Tauri 2 with a Rust backend and a SolidJS frontend. Install Rust (MSVC), Node.js 24+, pnpm and the Visual Studio C++ Build Tools, then:

```powershell
git clone https://github.com/Rishik15/cue.git
cd cue
./scripts/cue.ps1 setup
./scripts/cue.ps1 dev
```

`./scripts/cue.ps1 help` lists every mode (check, test, build, release, bench, smoke). Run `check` before a pull request; add `-Deps` when dependencies change. From WSL, `scripts/wsl/winsync <mode>` runs the same modes on Windows (`scripts/wsl/winsync.env.example` shows the setup).

## Code

Keep Windows calls inside `src-tauri/src/platform/` and check untrusted text where it enters. Vocabulary is in [GLOSSARY.md](GLOSSARY.md); the linters enforce size limits.
