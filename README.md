# GlowDiary — Food Diary MCP Server

Track your meals, nutrition, and daily goals. An MCP stdio server written in Rust.

**Current version:** [v$(cat VERSION)](https://github.com/koriwi/glowdiary/releases)

## Quick Start

```bash
# Run directly
glowdiary --db-path ~/.glowdiary/data.db

# In your MCP client config (Claude Desktop, etc.)
{
  "mcpServers": {
    "glowdiary": {
      "command": "/path/to/glowdiary",
      "args": ["--db-path", "/path/to/data.db"]
    }
  }
}
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `register_user` | Create user with UUID + default goals (2000 kcal) |
| `set_goals` | Set daily kcal/fat/protein/carbs targets |
| `get_goals` | Get current goals |
| `add_meal` | Log a meal with raw nutrition values |
| `add_meal_from_food` | Log a meal from a barcode + grams (auto-calculates) |
| `get_meal` | Single meal by UUID |
| `get_meals_by_day` | All meals for a day (sorted by eaten_at) |
| `get_meals_by_week` | All meals for an ISO week |
| `delete_meal` | Delete a meal |
| `get_stats` | Daily totals vs goals with percentages |
| `get_weekly_stats` | Weekly totals, averages, per-day breakdown |
| `search_food` | Search Open Food Facts (per-100g + serving sizes) |
| `lookup_barcode` | Look up a product by barcode |

## Docker

Always pin to a specific version for reproducible builds:

```dockerfile
FROM debian:bookworm-slim AS glowdiary
ARG GLOWDIARY_VERSION=0.1.0
ADD https://github.com/koriwi/glowdiary/releases/download/continuous/glowdiary-x86_64-unknown-linux-gnu.tar.gz /tmp/
RUN tar xzf /tmp/glowdiary-x86_64-unknown-linux-gnu.tar.gz -C /usr/local/bin/ && \
    chmod +x /usr/local/bin/glowdiary && \
    rm /tmp/glowdiary-x86_64-unknown-linux-gnu.tar.gz
```

The `continuous` tag always points to the latest build. For stability, pin to a
[release tag](https://github.com/koriwi/glowdiary/releases) instead.

## Versioning

This project follows [SemVer](https://semver.org/). The current version is in
[`VERSION`](VERSION) and [`Cargo.toml`](Cargo.toml).

To bump the version:

```bash
./scripts/bump-version.sh patch   # 0.1.0 → 0.1.1 (bug fixes)
./scripts/bump-version.sh minor   # 0.1.0 → 0.2.0 (new features)
./scripts/bump-version.sh major   # 0.1.0 → 1.0.0 (breaking changes)
```

## Working with pi

When using [pi](https://github.com/mariozechner/pi) to work on this project,
append the project rules so pi always bumps the version and commits:

```bash
pi --append-system-prompt .pi/APPEND_SYSTEM.md
```

## Build from Source

```bash
cargo build --release
./target/release/glowdiary --help
```
