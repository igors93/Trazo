# Contributing to Trazo

Thanks for your interest in Trazo! Stories from real building sites, bug reports, ideas, plain-language text, documentation, translations and code are all welcome. This guide explains how to get set up and what a good contribution looks like.

By taking part you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Who we build for

Read [the proposal](docs/proposta.md) first. Trazo exists so that a lack of money and technical knowledge does not leave a family unprotected against the risks of building its own home. Its first users are families, bricklayers and community groups who build without an engineer, not engineers who already know how to design. Every contribution is judged by one question: **does it make a self-built house safer, or easier to plan safely, for someone with no engineering training?**

Four rules follow from that:

- **MCP first.** The AI assistant is Trazo's operational centre: it designs, compares, reviews and organises for the family. Every capability must be usable through MCP, not only through a button.
- **Trazo contests the AI.** The assistant supplies inputs, never results. Never add a tool or setting that lets an assistant turn a red result green without evidence, and never weaken a check, a limit or a refusal to make a result pass (rule 10 of the [safety contract](ROADMAP.md#engineering-in-trazo-the-safety-contract)).
- **Write for the family.** Every message shown to users or returned by an MCP tool starts with one plain sentence that someone who never studied engineering understands; the formula and the standard come after. Explain technical terms.
- **Red is never hidden.** A change that hides, rounds away or silences a failing check is a bug, whatever else it improves.

## Ways to contribute

- **Tell us about a real building site.** What went wrong, what was confusing, which mistakes you see most. Open a [feature request](https://github.com/igors93/Trazo/issues/new?template=feature_request.yml) and choose *Families building their own home* as the audience.
- **Report a bug** with the [bug report form](https://github.com/igors93/Trazo/issues/new?template=bug_report.yml). Steps to reproduce and a screenshot or a small project file help a lot.
- **Suggest a feature** with the [feature request form](https://github.com/igors93/Trazo/issues/new?template=feature_request.yml). Check the [roadmap](ROADMAP.md) first; your idea may already have an item number.
- **Improve the docs and the interface text**: [the proposal](docs/proposta.md), the [user guide](docs/user-guide.md), the [MCP guide](docs/mcp.md) and the [architecture notes](docs/architecture.md). Simpler wording in the interface is as valuable as new code.
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

- Code, identifiers, comments, commit messages, technical docs and issues are in **English**. Issues in Portuguese are welcome too, especially from people building their own home.
- Text shown to users, the names and messages of MCP tools, the MCP server instructions and [the proposal](docs/proposta.md) are in **Brazilian Portuguese**. Use correct accents (`Cômodo`, not `Comodo`) and plain words: write for someone who never studied engineering.

### Code

- Keep `src/domain` free of UI types. Rules that both the screen and the exports need, such as label placement, belong in the domain.
- Edits are transactional. Clone the project, apply the change, validate it, and record one undo step only if it succeeds. Follow `record_change` in `src/app/undo.rs`.
- Engineering results are computed in the domain, never accepted from a tool's arguments. A new hypothesis (a soil value, a material strength, a load) needs a conservative default and must be listed among the assumptions.
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
