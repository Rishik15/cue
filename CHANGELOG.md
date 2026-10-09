# Changelog

All notable changes are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org/) (pre-1.0, so minor versions may change behaviour).

## [Unreleased]

## [0.1.0] - 2026-10-09

First milestone (M0, the app shell). Windows only.

### Added
- Tray app: single instance, launch at login, right-click menu (Open Cue, Settings, Quit) drawn natively with no web view, left click opens Settings.
- Settings window: frameless, six theme families with light and dark variants, General, Voice and About pages.
- Global dictation shortcut with Hold and Toggle modes and a recorder that supports Win combinations, side-specific modifiers, modifier-only keys and F-keys.
- Microphone capture with resampling to 16 kHz, device choice, keep-open time, maximum length and short-tap filtering.
- Native recording overlay with a live level meter, at the bottom or top of the screen.
- Text insertion into the focused app: typing for short text, a clipboard paste that restores the previous clipboard (all formats) for longer text, and a clear message when the target runs as administrator.
- Remove All Cue Data action, per-user Windows installer configuration, and a benchmark harness with a CI gate for idle memory, leaks and latency.

### Notes
- Speech recognition is not included yet, so dictation records and shows the overlay but does not produce text outside debug builds.
