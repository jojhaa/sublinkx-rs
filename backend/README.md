# Backend

This is the Rust backend for `sublinkx-rs`.

## Current Scope

Phase 1 bootstrap includes:

- Axum application entry
- basic config loading from environment
- health and version endpoints
- protocol capability placeholders
- first SQLite migration draft
- SQLite startup migration
- bootstrap admin creation
- JWT login and current-user endpoints

## Quick Start

1. Copy `.env.example` to `.env`
2. Adjust environment values if needed
3. Run:

```powershell
cargo run
```
