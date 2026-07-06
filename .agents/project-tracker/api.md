---
sources:
  - "README.md"
  - "monitor-proxy/src/router.rs"
  - "monitor-proxy/src/handlers/**/*.rs"
  - "monitor-proxy/src/dtos/**/*.rs"
  - "monitor-proxy/src/middlewares/**/*.rs"
  - "monitor-common/src/live.rs"
---
# API Reference

## Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/auth/login` | Public | Authenticates against Phira credentials and returns a proxy JWT. |
| `GET` | `/auth/me` | Bearer token / auth middleware | Returns the current authenticated user's profile. |
| `GET` | `/ws/live` | Auth token accepted by auth middleware | Upgrades to a binary WebSocket for live room monitor commands and events. |
| `GET` | `/visited` | Public | Returns visited Phira user count and, optionally, user IDs. |
| `GET` | `/chart/{id}` | Public | Streams a processed chart binary as `application/octet-stream`. |
| `GET` | `/rooms/info` | Public | Returns all active rooms. |
| `GET` | `/rooms/info/{id}` | Public | Returns details for one room ID. |
| `GET` | `/rooms/user/{id}` | Public | Returns the room containing a given Phira user ID, if any. |
| `GET` | `/rooms/listen` | Public | Opens an SSE stream of room lifecycle events. |

## Request / Response

### `POST /auth/login`

**Request:**
```json
{
  "email": "string",
  "password": "string"
}
```

**Response:**
```json
{
  "token": "jwt string"
}
```

### `GET /auth/me`

**Auth:** authenticated through `middlewares::require_auth`.

**Response:**
```json
{
  "id": 0,
  "username": "string",
  "phira_avatar": "string or null",
  "phira_id": 0,
  "phira_rks": 0.0,
  "phira_username": "string",
  "register_time": "ISO-8601 timestamp",
  "last_login_time": "ISO-8601 timestamp"
}
```

### `GET /visited?count_only=false`

**Response:**
```json
{
  "count": 0,
  "users": [
    { "phira_id": 0 }
  ]
}
```

When `count_only=true`, the `users` field is omitted.

### `GET /rooms/info`

**Response:**
```json
{
  "rooms": [
    {
      "name": "room id",
      "data": "phira_mp_common::RoomData JSON shape"
    }
  ],
  "total": 1
}
```

### `GET /rooms/info/{id}` and `GET /rooms/user/{id}`

Return a `RoomInfoResponse` JSON value when a room is found. User room lookup may return `null` depending on service result.

### `GET /rooms/listen`

Returns `text/event-stream` with keepalive every 10 seconds. Documented event names include `create_room`, `update_room`, `join_room`, `leave_room`, and `new_round`; payloads are JSON room lifecycle data.

### `GET /chart/{id}`

Returns `application/octet-stream` containing bincode-encoded chart data consumed by `monitor-client`.

### `GET /ws/live`

Upgrades to WebSocket. The browser-side `GameMonitor` sends binary-encoded `WsCommand` values such as join, leave, and ready. The server sends binary-encoded live events consumed by `GameMonitor`.

## Rate Limiting

No rate limiting middleware or per-endpoint throttle policy is configured in the repository.

## Pagination

No paginated endpoint exists. `/visited` supports `count_only`, but not limit/offset pagination.
