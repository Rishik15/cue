# Patch notes

This directory is a copy of [handy-keys](https://github.com/handy-computer/handy-keys) 0.3.4 (MIT, see `LICENSE`) with changes for Cue, all in `src/platform/windows/listener.rs` (the third also re-exported from `src/lib.rs`). The examples and tests of the original were left out.

1. No idle wakeups. The long-lived hook thread blocks until a message arrives instead of waking every 10 ms, and shutdown posts `WM_QUIT` to wake it, at `THREAD_PRIORITY_HIGHEST` (Windows drops a late hook without a word). This removes the only periodic wakeup the library caused while Cue sits idle in the tray.
2. A blocked hotkey key stays blocked for the whole hold (`latch`). The library decides per event whether it matches a blocked hotkey, modifiers included. People who hold to talk often let go of the modifier before the key. The held key's auto-repeat then stopped matching and reached the focused app as typed characters, such as a row of spaces or backticks. Now a blocked key press is remembered, and its repeats and its release are blocked too. Entries for keys that are no longer physically held are dropped on the next event, so a key can never stay dead. The first press of a key that arrives before its modifier cannot be recalled, so it still types once.
3. Cue's own keystrokes are invisible to the hook. Input Cue injects (a Ctrl+C to copy a selection, a Ctrl+V, typed text) carries `CUE_INPUT_MARK` in `dwExtraInfo` and passes through the keyboard hook without touching the tracked modifiers, matching a hotkey or triggering reconciliation. That lets Cue lift the modifiers a shortcut is still holding for the length of its own keystrokes and put them back (`platform/win/input.rs`), so a shortcut acts the moment it is pressed instead of after its keys are released. `set_lifting` pauses reconciliation (the async key state is the lifted one during that time) and `held_modifiers` gives the modifiers the hook sees physically held, from any thread.

Unchanged from upstream, and relied on: when Windows unlocks the session, a console or remote session connects, or the machine resumes from sleep, the hook thread re-installs both hooks (`reinstall_hooks` in `listener.rs`, triggered by the watcher window's `WM_WTSSESSION_CHANGE` and `WM_POWERBROADCAST`), because Windows can silently drop a low-level hook. It runs on the message the thread already wakes for, so it costs nothing at idle. The upstream comment about a short polling timeout in the message loop was removed because patch 1 made it false.

## Patch: `try_update`

`LIFTING.fetch_update` was renamed `try_update` in current Rust and warned on every build; `listener.rs` calls `try_update` now. Behavior is the same.

## Patch: hook install errors

`spawn` in `listener.rs` returns an error, with Windows' own reason, when either hook cannot be installed. It used to print to stderr and end the thread, and the failure showed up later as a dead channel (Cue said its shortcuts "stopped working" with no cause, and a GUI app has no stderr).


## Patch: polling listener for recordings

`KeyboardListener::new_recording()` (Windows) starts a listener like upstream handy-keys and Handy's shortcut recorder do: its own hook thread at normal priority that wakes every 10 ms. It also swallows every key (`capture` in `HookContext`, with the menu mask for Win and Alt, as PowerToys does while it records), so Win does not open the Start menu and no shortcut of another program fires while one is being recorded; Escape still reaches the recorder. Cue creates one per shortcut recording (`shortcut/recorder.rs`) and drops it when the recording ends, so nothing wakes while idle. On the owner's laptop the long-lived hook stopped being called during recordings (74 successful re-installs, no calls) while Handy's recorder worked on the same machine; this replaces `rearm_hooks`.
