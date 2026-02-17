# phira-web-monitor

包含观战系统（未完成）、用户系统（套壳 Phira）、房间查询系统。

下面所有命令假设初始时工作目录在项目根目录。

## monitor-common

### 功能

用于 monitor-client 和 monitor-proxy 的通用组件库。

## monitor-client

### 功能

观战的网页前端组件。目前仅实现谱面渲染、播放功能。

### 使用

使用下面的命令编译：

```
cd monitor-client/
wasm-pack build --out-dir ../web/pkg --target web
```

## monitor-proxy

### 功能

一个和服务器的代理层，实现的接口如下：

#### `GET /chart/{id}`

**说明**：获取 `id` 谱面的二进制数据，用于 monitor-client。

**响应格式**：`application/octet-stream`。谱面二进制数据。

#### `GET /rooms/info`

**说明**：获取当前所有房间列表。

**响应格式**：`application/json`。房间列表数组。

```json
[
  {
    "name": "u123", // 房间 ID
    "data": {
      // 房间数据对象
      "host": 123, // 房主 ID (-1 表示无房主)
      "users": [123, 456], // 房间内用户 ID 列表
      "lock": false, // 是否上锁
      "cycle": false, // 是否轮换房主
      "chart": 1001, // 选中谱面 ID (null 表示未选)
      "state": "PLAYING" // 状态: SELECTING_CHART, WAITING_FOR_READY, PLAYING
    }
  }
]
```

#### `GET /rooms/info/{id}`

**说明**：获取指定 `id` 房间的详细信息。

**响应格式**：`application/json`。房间数据对象（Schema 同上 `data` 字段）。

#### `GET /rooms/user/{id}`

**说明**：获取指定用户 `id` 所在的房间信息。

**响应格式**：`application/json`。房间数据对象，如果用户不在房间中则为 `null`。

#### `GET /rooms/listen`

**说明**：监听房间列表的实时更新事件 (SSE)。

**响应格式**：`text/event-stream`。

事件类型：

- `create_room`: `{"room": "id", "data": <RoomData>}`
- `update_room`: `{"room": "id", "data": <PartialRoomData>}`
- `join_room`: `{"room": "id", "user": <UserId>}`
- `leave_room`: `{"room": "id", "user": <UserId>}`
- `start_round`: `{"room": "id"}`
- `player_score`: `{"room": "id", "record": <RecordData>}`

**RecordData Schema**:

```json
{
  "id": 1,
  "player": 123,
  "score": 1000000,
  "perfect": 100,
  "good": 0,
  "bad": 0,
  "miss": 0,
  "max_combo": 100,
  "accuracy": 1.0,
  "full_combo": true,
  "std": 0.0,
  "std_score": 0.0
}
```

#### `POST /auth/login`

**说明**：登录 Phira 账号（代理登录）。

**请求格式**：`application/json`。

```json
{
  "email": "user@example.com",
  "password": "password"
}
```

**响应格式**：`application/json`。

```json
{
  "message": "login success"
}
```

（设置 `hsn_auth` Cookie）

#### `GET /auth/me`

**说明**：获取当前登录用户的个人信息。

**响应格式**：`application/json`。

```json
{
  "id": 123,
  "username": "User",
  "phira_id": 123,
  "phira_username": "User",
  "phira_avatar": "avatar_url", // 可能为 null
  "phira_rks": 15.5,
  "register_time": "2023-01-01T00:00:00Z",
  "last_login_time": "2023-01-02T00:00:00Z"
}
```

### 使用

**在生产环境运行时，必须在环境变量设置 secret key**：

```
export HSN_SECRET_KEY=<some_random_secret_key>
```

使用下面的命令，自动编译并运行：

```cpp
cargo run --bin monitor-proxy -- <args>...
```

使用 `--help` 可以查询可用的选项。

## web

### 功能

用于测试谱面播放的网页。

### 使用

需要先编译 `monitor-client`。使用下面的命令运行**开发服务器**：

```
npm run dev
```

如果 monitor-proxy 没有运行在默认端口上，需要在 `vite.config.ts` 中修改 `proxy`。

使用 vite 项目的标准方式配置生产服务器，注意要像 `vite.config.ts` 中一样设置反向代理。~~虽然我觉得这个测试网页也不应该上生产服务器就是了~~。
