# Patch notes

This directory is a copy of [handy-keys](https://github.com/handy-computer/handy-keys) 0.3.4 (MIT, see `LICENSE`) with two changes for Cue, both in `src/platform/windows/listener.rs`. Examples and tests of the original were left out.

1. **No idle wakeups.** The keyboard hook thread blocks until a message arrives instead of waking every 10 ms, and shutdown posts `WM_QUIT` to wake it. This removes the only periodic wakeup the library caused while Cue sits idle in the tray.
2. **A blocked hotkey key stays blocked for the whole hold (`latch`).** The library decides per event whether it matches a blocked hotkey, modifiers included. Hold-to-talk users often let go of the modifier before the key; the held key's auto-repeat then no longer matched and reached the focused app as typed characters (a row of spaces or backticks). A blocked key press is now remembered and its repeats and its release are blocked too. Entries for keys no longer physically held are dropped on the next event, so a key can never stay dead. The first press of a key that arrives before its modifier cannot be recalled and still types once.
