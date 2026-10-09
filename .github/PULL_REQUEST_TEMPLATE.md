## What and why

<!-- One or two sentences. Link the issue if there is one. -->

## Checklist

- [ ] `pnpm check`, `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test` pass
- [ ] New logic has a test; UI changes follow the existing components in `src/ui/`
- [ ] Nothing in the idle path (timers, polling, resident helpers) was added, or the benchmark shows it is free
- [ ] Docs and `CHANGELOG.md` are updated when behaviour changes
