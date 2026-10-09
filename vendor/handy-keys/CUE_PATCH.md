# Patch notes

This directory is a copy of [handy-keys](https://github.com/handy-computer/handy-keys) 0.3.4 (MIT, see `LICENSE`) with one change for Cue: on Windows the keyboard hook thread blocks until a message arrives instead of waking every 10 ms, and shutdown posts `WM_QUIT` to wake it. This removes the only periodic wakeup the library caused while Cue sits idle in the tray. Examples and tests of the original were left out.
