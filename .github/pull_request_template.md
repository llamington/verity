## What and why

## How I verified it

- [ ] `cargo fmt`, `cargo clippy -D warnings` and `cargo test --workspace` pass (with the data blobs built)
- [ ] `npm run check`, `npm run lint` and `npx vitest --run` pass in `web/` (if the web app changed)
- [ ] Legality or generation changes have a parity test against the PKHeX oracle
