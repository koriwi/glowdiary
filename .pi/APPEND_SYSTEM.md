# GlowDiary — Project Rules

When working on this project, follow these rules strictly.

## Versioning (SemVer)

This project uses [SemVer](https://semver.org/). The current version is in `VERSION` (also in `Cargo.toml`).

**You MUST bump the version on every meaningful change BEFORE committing.**

| Change | Bump |
|--------|------|
| Bug fix, docs, refactoring | `patch` (0.1.0 → 0.1.1) |
| New feature, new tool | `minor` (0.1.0 → 0.2.0) |
| Breaking API change | `major` (0.1.0 → 1.0.0) |

Run `./scripts/bump-version.sh [patch|minor|major]` to update both `VERSION` and `Cargo.toml`.

**Never commit without bumping the version if you changed any source code.**

## Commit Discipline

After bumping the version and making changes:

1. `git add -A`
2. `git commit -m "semver_bump: <descriptive message>"`
3. `git push`

Always verify `cargo check` passes before committing.

## MCP Tool Changes

When adding or modifying MCP tools:
- Update `tools/mod.rs` (the `#[tool_router]` impl block)
- Tools return `String` (JSON-formatted)
- All DB access goes through `GlowDiary::with_db()`
- Open Food Facts lookups through `fddb::search()` / `fddb::lookup_barcode()`

## Project Layout

```
src/
├── main.rs        # tokio main, clap CLI
├── error.rs       # thiserror error types
├── fddb.rs        # Open Food Facts API client (retry wrapper included)
├── db/
│   ├── mod.rs     # SQLite init + migrations
│   ├── users.rs   # user CRUD
│   ├── meals.rs   # meal CRUD + daily/weekly stats
│   └── goals.rs   # goal CRUD
└── tools/
    └── mod.rs     # GlowDiary struct, tools, ServerHandler
```
