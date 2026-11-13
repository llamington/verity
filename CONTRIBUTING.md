# Contributing to Verity

Thanks for helping. Verity edits people's saves, so the bar is that every change stays legal (as
PKHeX judges it) and never corrupts a save. Bug reports with the save that triggered them are the
most useful contribution of all; see [Reporting bugs](#reporting-bugs).

## Setup

| Tool | Needed for | Notes |
| --- | --- | --- |
| Rust (rustup) | everything | `rust-toolchain.toml` pins the nightly; rustup installs it on first use. |
| .NET 9 SDK | data blobs, oracle tests | `tools/pkhex-oracle` pulls `PKHeX.Core` from NuGet. |
| Node (version in `.node-version`) | web app, commit hook | Run `npm install` at the repo root. |
| `wasm-pack`, `wasm32-unknown-unknown` target | web app | `cargo install wasm-pack`, `rustup target add wasm32-unknown-unknown`. |
| [BlocksDS](https://blocksds.skylyrac.net) | DS build | Set `BLOCKSDS` if it isn't at `/opt/blocksds/core`. |
| CMake, a C++17 compiler, Python 3 | DS UI tests | CMake fetches melonDS 1.1 unless `MELONDS_DIR` points at a checkout. |

Then build the data blobs once. They come from PKHeX and are gitignored:

```sh
npm install                # web workspace + the commit-msg hook
scripts/build-data.sh      # data/blobs/*.bin (set DOTNET_ROOT if dotnet isn't on PATH)
```

## Building

```sh
cargo build --workspace    # core, bindings, tools
cargo build-ds             # the core as an ARM9 static library
make nds                   # nds/verity.nds (needs BlocksDS)
cd web && npm run dev      # the web app on localhost (builds the WASM first)
```

## Tests

Run these before opening a pull request. CI runs the same checks.

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd web && npm run check && npm run lint && npx vitest --run
```

Test logic at the lowest layer it lives in: the Rust core first, then the WASM binding's Rust
tests (`crates/verity-wasm/tests`), then Svelte components through Storybook play functions. Keep
Playwright (`npm run test:e2e`) for end-to-end journeys.

**Tests that need data skip without it.** Many core tests compare against PKHeX using the blobs
and a corpus of real saves, and they return early when the files are missing. Build the blobs
first. The real-save corpus (`corpus/`) isn't distributed, because those saves belong to their
owners; `docs/research/synthetic-corpus.md` describes generating synthetic saves with the oracle.
`scripts/oracle-smoke.sh` writes the PKHeX dumps some tests read.

Long randomized parity runs are `#[ignore]`d; run them with `cargo test -- --ignored` when you
change legality or generation code.

DS UI tests: `make emu-test` runs the headless emulator against reference screens. See
`docs/research/emu-harness.md`.

Coverage is local only: `cargo coverage` (needs `cargo-llvm-cov`) and `npm run coverage` in `web/`.

## Code rules

- **PKHeX is the reference.** Legality and generation code ports PKHeX (tag 25.12.21). Cite the
  PKHeX file you ported from, and back a behavior change with a parity test against the oracle.
  Where Verity deliberately differs, record it in `docs/discrepancies.md`.
- **No `unsafe` outside `crates/verity-ffi`.** The workspace forbids it. In `verity-ffi`, each
  unsafe operation gets its own block and a `// SAFETY:` comment tied to the function's
  `# Safety` contract.
- **The core stays `no_std`**, does no I/O and works on byte buffers, so the DS and the web share it.
- **TypeScript stays strict.** No `any`, no `as` type assertions (`as const` is fine), and no
  `@ts-ignore`. Narrow with a check instead; the lint config enforces this.
- **No Nintendo assets in the repo or a release.** The DS reads fonts and graphics from the
  user's own ROM at runtime, and the web app hotlinks sprites.

## Commits and pull requests

Commits follow [Conventional Commits](https://www.conventionalcommits.org): `feat(web): …`,
`fix(core): …`. Subjects start lowercase. The scopes are listed in `commitlint.config.js`. The
commit-msg hook (installed by `npm install`) and CI both check this.

Keep each pull request to one change, with tests, and say how you verified it.

## Reporting bugs

Open an issue with the game, what you did, and what happened. If a save is involved, attach it
(zipped) or say you can share it privately. A save that PKHeX and Verity disagree on is a bug
either way. For security issues, see [SECURITY.md](SECURITY.md).

## Licence

Verity is GPLv3. By contributing, you agree that your contribution is licensed under the same terms.
