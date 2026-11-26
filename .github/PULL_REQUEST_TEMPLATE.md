## What does this change?

<!-- A short description of the change and why it is needed. -->

Closes #<!-- issue number -->
Roadmap: #<!-- item number, if any -->

## How was it tested?

<!-- Tests added or changed, and what you checked by hand in the app. -->

## Screenshots

<!-- Before and after, for any change people can see. Delete this section otherwise. -->

## Who does this protect?

<!-- Trazo is built for families who build without an engineer (docs/proposta.md). How does this change make a self-built house safer, or easier to plan safely, for someone with no engineering training? -->

## Checklist

- [ ] Someone without engineering training can understand the new text and messages (plain sentence first)
- [ ] No check, limit or refusal is weakened, and no red result can be hidden or turned green without evidence (by a person or an AI assistant)
- [ ] The capability is usable through MCP, not only through the interface
- [ ] `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test` pass
- [ ] New behaviour has tests, and bug fixes have a test that fails without the fix
- [ ] Interface text is in Brazilian Portuguese; code and comments are in English
- [ ] Saved project files stay compatible (`#[serde(default)]`, migration tested if needed)
- [ ] Docs updated (README, `docs/`, `CHANGELOG.md`) when behaviour changes
- [ ] New or changed MCP tools are listed in `docs/mcp.md`
