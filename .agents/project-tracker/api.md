---
sources:
  - "monitor-proxy/src/router.rs"
  - "monitor-proxy/src/handlers/**/*.rs"
  - "monitor-proxy/src/middlewares/**/*.rs"
  - "monitor-proxy/src/dtos/*.rs"
  - "monitor-common/src/live.rs"
  - "README.md"
---

# API Reference

## Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/auth/login` | No | Proxy login to the official Phira API and return a local JWT |
| `GET` | `/auth/me` | Yes | Return the current user profile derived from the stored upstream token |
| `GET` | `/chart/{id}` | No | Stream a serialized chart payload (`application/octet-stream`) for WASM consumption |
| `GET` | `/rooms/info` | No | Return the current room list |
| `GET` | `/rooms/info/{id}` | No | Return one room by room ID |
| `GET` | `/rooms/user/{id}` | No | Return the room containing a given user, or `null` |
| `GET` | `/rooms/listen` | No | SSE stream of room updates, plus initial replay |
| `GET` | `/visited` | No | Return count or list of visited users |
| `GET` | `/ws/live` | Yes | Authenticated WebSocket for live gameplay monitoring |
| `GET` | `/hot_rank/{time_range}` | No | Paginated hot-chart ranking for `hour`, `day`, `week`, or `month` |
| `GET` | `/chart_rank/{chart_id}` | No | Rank snapshot for one chart across all supported time windows |

Auth notes:

- `/auth/me` expects `Authorization: Bearer <token>`.
- `/ws/live` is protected by the same middleware and can use either the `Authorization` header or `?token=<jwt>` query parameter during WebSocket upgrade.

## Request / Response

### `POST /auth/login`

**Request:**
```json
{
  "email": "user@example.com",
  "password": "plain-text password"
}
```

**Response:**
```json
{
  "token": "local-jwt"
}
```

### `GET /auth/me`

**Response:**
```json
{
  "id": 123,
  "username": "display name",
  "phira_avatar": "https://...",
  "phira_id": 456,
  "phira_rks": 15.37,
  "phira_username": "official profile name",
  "register_time": "2026-01-01T00:00:00Z",
  "last_login_time": "2026-06-16T00:00:00Z"
}
```

### `GET /rooms/info`

**Response shape:**
```json
{
  "total": 2,
  "rooms": [
    {
      "name": "abcd",
      "data": {
        "host": 1,
        "users": [1, 2],
        "lock": false,
        "cycle": false,
        "chart": 1234,
        "state": "PLAYING",
        "rounds": []
      }
    }
  ]
}
```

### `GET /visited?count_only=true|false`

**Response shape:**
```json
{
  "count": 42,
  "users": [
    { "phira_id": 1001 }
  ]
}
```
`users` is omitted when `count_only=true`.

### `GET /hot_rank/{time_range}?page=<u32>&per_page=<u32>`

**Response shape:**
```json
{
  "last_chart_list_update": "2026-06-16T00:00:00Z",
  "last_record_update": "2026-06-16T00:00:00Z",
  "page": 1,
  "per_page": 20,
  "time_range": "day",
  "total_results": 200,
  "results": [
    {
      "chart_id": 1234,
      "increase": 87
    }
  ]
}
```

### `GET /chart_rank/{chart_id}`

**Response shape:**
```json
{
  "chart_id": 1234,
  "ranks": {
    "hour": { "increase": 4, "rank": 12, "last_update": "2026-06-16T00:00:00Z" },
    "day": { "increase": 21, "rank": 5, "last_update": "2026-06-16T00:00:00Z" },
    "week": { "increase": 90, "rank": 7, "last_update": "2026-06-16T00:00:00Z" },
    "month": { "increase": 310, "rank": 9, "last_update": "2026-06-16T00:00:00Z" }
  }
}
```

### `GET /ws/live`

The WebSocket transports binary `monitor_common::live::LiveEvent` payloads. Browser commands are binary `WsCommand` values:

- `Join { room_id }`
- `Leave`
- `Ready`

Server events include:

- `Authenticate`
- `Join`
- `Leave`
- `Touches`
- `Judges`
- `StateChange`
- `UserJoin`
- `UserLeave`
- `Message`

## Rate Limiting

| Window | Limit | Behavior |
|--------|-------|----------|
| N/A | Not implemented in current code | Clients are accepted until upstream or infrastructure limits apply |

## Pagination

- `/hot_rank/{time_range}` uses explicit `page` and `per_page` query parameters and returns `total_results`.
- Room, visited-user, chart, SSE, and WebSocket endpoints are not paginated.
