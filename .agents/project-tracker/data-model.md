---
sources:
  - "monitor-proxy/src/entity/*.rs"
  - "monitor-proxy/src/services/chart_service.rs"
  - "monitor-proxy/src/services/room_service.rs"
  - "monitor-proxy/src/services/topchart_service.rs"
  - "monitor-proxy/migration/src/*.rs"
  - "monitor-proxy/src/config.rs"
---

# Data Model

## Entities

| Entity | Key fields | Notes |
|--------|-----------|-------|
| `visited_users` | `phira_id` (PK) | Deduplicated set of users observed creating or joining rooms through the mp monitor path |
| `chart_records` | `chart_id` (PK part), `timestamp` (PK part), `count` | Time-series snapshots of official chart record counts, used as the raw input for ranking windows |
| `chart_statistics` | `chart_id` (PK), `count_hour`, `count_day`, `count_week`, `count_month` | Materialized rolling deltas used by the hot-rank endpoints |
| Disk chart cache | Chart ID + upstream `chartUpdated` marker | Stored as `.bin` and `.meta` files under `cache_dir`; not DB-backed but part of the runtime data model |

## Relationships

| From | To | Cardinality | Description |
|------|----|------------|-------------|
| `chart_records.chart_id` | `chart_statistics.chart_id` | 1:N raw snapshots to 1 derived aggregate row | Statistics are recomputed from the latest record history for each chart |
| `chart_statistics.chart_id` | Official Phira chart ID | 1:1 logical link | Rows correspond to upstream chart identifiers, not a local chart table |
| `visited_users.phira_id` | Official Phira user ID | 1:1 logical link | Local store tracks only the remote user ID |

## Schema Migrations

| Aspect | Detail |
|--------|--------|
| Tool | SeaORM Migration |
| Location | `monitor-proxy/migration/src/` |
| Strategy | Incremental Rust migrations executed automatically at backend startup via `Migrator::up(&db, None)` |

Current migration chain:

1. `m20220101_000001_create_table` creates `visited_users`.
2. `m20260326_075306_support_topchart` creates `chart_records`, `chart_statistics`, and ranking indexes.

## Caching

| Cache | Strategy | TTL | Invalidation |
|-------|---------|-----|-------------|
| Chart binary cache | Cache-aside on first request | No fixed TTL | Invalidate when upstream `chartUpdated` differs from the stored metadata |
| Room snapshot/event cache | In-memory replay buffer | Short-lived, process memory only | Replaced on fresh room sync; subscribers also get a broadcast stream |
| Visited-user queue | In-memory write buffer before DB flush | Best-effort, flushed on `/visited` reads | Cleared after successful insert-many with `ON CONFLICT DO NOTHING` |
| Ranking raw history | Rolling DB history | 35-day retention in `TopChartService` | Old rows are deleted during each ranking refresh |
