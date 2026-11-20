# Contributing to Trazo

Thanks for your interest in Trazo! Bug reports, ideas, documentation, translations and code are all welcome. This guide explains how to get set up and what a good contribution looks like.

By taking part you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Ways to contribute

- **Report a bug** with the [bug report form](https://github.com/igors93/Trazo/issues/new?template=bug_report.yml). Steps to reproduce and a screenshot or a small project file help a lot.
- **Suggest a feature** with the [feature request form](https://github.com/igors93/Trazo/issues/new?template=feature_request.yml). Check the [roadmap](ROADMAP.md) first; your idea may already have an item number.
- **Improve the docs**: the [user guide](docs/user-guide.md), the [MCP guide](docs/mcp.md) and the [architecture notes](docs/architecture.md).
- **Write code**: pick a roadmap item or an issue labelled `good first issue`.
- **Report a vulnerability** privately, as described in [SECURITY.md](SECURITY.md). Never in a public issue.

For anything bigger than a small fix, open an issue first so we can agree on the approach before you invest time.

## Development setup

1. Install the stable Rust toolchain with [rustup](https://rustup.rs) (edition 2024 needs Rust 1.85 or newer).
2. Clone and run:

   ```sh
   git clone https://github.com/igors93/Trazo.git
   cd Trazo
   cargo run --release
   ```

3. On Linux, install the development packages for X11/Wayland, OpenGL and GTK first. On Debian or Ubuntu:

   ```sh
   sudo apt install libxkbcommon-dev libgl1-mesa-dev libgtk-3-dev
   ```

Trazo is developed and tested mainly on Windows. Reports from Linux and macOS are especially welcome.

To understand the code base, start with [docs/architecture.md](docs/architecture.md).

## Guidelines

### Language

- Code, identifiers, comments, commit messages, docs and issues are in **English**.
- Text shown to users, and the names and messages of MCP tools, are in **Brazilian Portuguese**, like the rest of the interface. Use correct accents (`Cômodo`, not `Comodo`).

### Code

- Keep `src/domain` free of UI types. Rules that both the screen and the exports need, such as label placement, belong in the domain.
- Edits are transactional. Clone the project, apply the change, validate it, and record one undo step only if it succeeds. Follow `record_change` in `src/app/mod.rs`.
- New fields in saved types need `#[serde(default)]`, and a migration test when the meaning of old files changes.
- New MCP tools need four things: an entry in the catalogue in `src/mcp/protocol.rs` (with accurate read-only and destructive hints), the implementation in `src/app/mcp_tools`, a test, and a line in [docs/mcp.md](docs/mcp.md).
- UI work uses the colours and spacing in `src/app/theme.rs` and the shared widgets in `src/app/ui/widgets.rs`. Check it at different window sizes.
- Prefer small, readable functions with doc comments that explain *why*. Discuss new dependencies in an issue before adding them.

### Tests

- Put unit tests next to the code (`#[cfg(test)] mod tests`). Geometry changes deserve [`proptest`](https://docs.rs/proptest) properties.
- Every bug fix comes with a test that fails without the fix.
- Run the same checks as CI before pushing:

  ```sh
  cargo fmt --all
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test --all-targets --all-features
  ```

### Commits

- One logical change per commit, with a short imperative summary.
- Use a type prefix: `feat:`, `fix:`, `style:`, `refactor:`, `docs:`, `test:`, `chore:`.
- Reference the roadmap item or issue when there is one, for example `feat: import DXF walls and layers (#38)`.

## Pull requests

1. Fork the repository and create a branch from `main`.
2. Make your change, with tests and docs.
3. Fill in the pull request template. For UI changes, include before and after screenshots.
4. Make sure CI passes. A maintainer will review; please be patient and responsive to feedback.

Small, focused pull requests are reviewed fastest.

## License

Trazo is dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
