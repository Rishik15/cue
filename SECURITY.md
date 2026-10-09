# Security policy

## Supported versions

Cue is in early development and has no releases yet. Only the `main` branch receives fixes.

## Reporting a vulnerability

Please do not open a public issue for a security problem. Report it privately through GitHub's [private vulnerability reporting](https://github.com/Rishik15/cue/security/advisories/new) for this repository, or email rishikreddy.yesgari@gmail.com.

Include what you found, how to reproduce it, and the impact you expect. You will get an acknowledgement within a few days, and we will keep you informed while a fix is prepared. Please allow reasonable time to ship a fix before disclosing publicly.

## Scope and design notes

Cue processes text, audio and screenshots on the device. Network access is limited to explicit model downloads and update checks. Things worth reporting include code execution through crafted input, leaks of dictated or captured content (to logs, the clipboard history or the network), the app writing outside its documented data directories, and bypasses of its safeguards (for example inserting text into a password field or auto-submitting a message).
