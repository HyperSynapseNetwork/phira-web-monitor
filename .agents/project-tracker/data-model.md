---
sources:
  - "monitor-proxy/migration/src/**/*.rs"
  - "monitor-proxy/src/entity/**/*.rs"
  - "monitor-proxy/src/config.rs"
  - "monitor-proxy/src/services/*.rs"
  - "monitor-proxy/src/utils/**/*.rs"
---
# Data Model

## Entities

| Entity | Key fields | Notes |
|--------|------------|-------|
| `visited_users` | `phira_id: i32` primary key, no auto-increment | Tracks Phira user IDs that have visited/participated through this proxy. |

## Relationships

| From | To | Cardinality | Description |
|------|----|-------------|-------------|
| `visited_users` | External Phira user/profile data | External reference | `phira_id` refers to a Phira account ID, but no local foreign key exists. |

There are no checked-in relational models for rooms, charts, profiles, or live events. Those are fetched from external Phira services, represented as DTOs/shared protocol types, and cached/transformed as needed.

## Schema Migrations

| Aspect | Detail |
|--------|--------|
| Tool | SeaORM Migration 1.1 |
| Location | `monitor-proxy/migration/src/` |
| Current migration | `m20220101_000001_create_table.rs` creates `visited_users`. |
| Strategy | Backend startup calls `Migrator::up(&db, None)` after connecting to `DATABASE_URL`. |
| Supported backends | SeaORM features enable SQLite, PostgreSQL, and MySQL through SQLx runtimes. |

## Caching

| Cache | Strategy | TTL | Invalidation |
|-------|----------|-----|--------------|
| Chart disk cache | Local filesystem cache under `--cache-dir`, defaulting to `$HOME/.cache/hsn-phira`. | No explicit TTL found in config. | Manual deletion or service-specific cache logic. |
| In-flight chart work | Request deduplication for concurrent chart processing, described in backend entry comments. | Request lifetime. | Completes when chart processing finishes. |
| Browser resource pack | Loaded from `web/public/assets/respack/default` into WASM resource state. | Browser/session lifetime. | Page reload or explicit resource replacement. |
