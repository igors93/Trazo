## What does this change?

<!-- A short description of the change and why it is needed. -->

Closes #<!-- issue number -->
Roadmap: #<!-- item number, if any -->

## How was it tested?

<!-- Tests added or changed, and what you checked by hand in the app. -->

## Screenshots

<!-- Before and after, for any change people can see. Delete this section otherwise. -->

## Checklist

- [ ] `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test` pass
- [ ] New behaviour has tests, and bug fixes have a test that fails without the fix
- [ ] Interface text is in Brazilian Portuguese; code and comments are in English
- [ ] Saved project files stay compatible (`#[serde(default)]`, migration tested if needed)
- [ ] Docs updated (README, `docs/`, `CHANGELOG.md`) when behaviour changes
- [ ] New or changed MCP tools are listed in `docs/mcp.md`
