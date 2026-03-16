# HSN Phira Web Monitor

这是一个完整的基于 Web 的工具链，专为 Phira 设计，提供实时的多人观战、用户同步和房间查询功能。

本项目主要作为 Phira 多人游戏房间的代理和 Web 可视化层，允许用户直接在浏览器中观看实时渲染的谱面。

## 架构总览

本项目由 4 个协同工作的主要工作区（workspace）组成：

1. **`monitor-common`**：定义了跨网络层和 WebGL 渲染器使用的共享 Rust 数据结构、二进制解析工具和核心逻辑。
2. **`monitor-proxy`**：基于 Rust Axum 的服务器，作为官方 Phira 服务器和浏览器客户端之间的桥梁。它负责用户认证（JWT）、轮询房间列表、流式传输远程判定事件（SSE），以及提供谱面二进制文件。
3. **`monitor-client`**：本项目的 WebAssembly (WASM) 核心。使用 Rust 编写，解码 `bincode` 谱面数据，并利用 WebGL 原生计算并渲染 Phira 谱面。
4. **`web`**：一个现代的 Vue 3 + TypeScript 前端应用。它管理 UI 状态，建立 WebSocket 和 SSE 事件监听器，协调音频上下文（AudioContext），并为 WASM WebGL 引擎动态管理画布（Canvas）尺寸。

---

## 组件详情与 API 参考

### `monitor-proxy`

代理服务器，作为应用的主后端。

#### 数据格式定义

所有的接口交互均使用以下 TypeScript 接口定义为基础：

```typescript
// === 认证与用户信息相关 (Auth) ===

export interface LoginRequest {
  email: string;
  password: string;
}

export interface LoginResponse {
  token: string;
}

export interface ProfileResponse {
  id: number;
  username: string;
  phira_avatar: string | null;
  phira_id: number;
  phira_rks: number;
  phira_username: string;
  register_time: string; // ISO 8601 格式的时间字符串
  last_login_time: string; // ISO 8601 格式的时间字符串
}

// === 房间信息及列表相关 (Rooms) ===

export interface RoomListResponse {
  total: number; // 房间总数
  rooms: RoomInfoResponse[]; // 房间详细信息列表
}

export interface RoomInfoResponse {
  name: string; // 房间 ID 标识符
  data: RoomData;
}

export interface RoomData {
  host: number; // 房主 ID (-1 表示无房主)
  users: number[]; // 房间内用户 ID 列表
  lock: boolean; // 是否上锁
  cycle: boolean; // 是否轮换房主
  chart: number | null; // 选中的谱面 ID (null 表示未选)
  state: "SELECTING_CHART" | "WAITING_FOR_READY" | "PLAYING"; // 房间所处状态
  rounds: RoundData[]; // 房间历史对局列表
}

export interface RoundData {
  chart: number; // 该对局的谱面 ID (-1 表示无)
  records: RecordData[]; // 该对局的玩家成绩列表
}

export interface RecordData {
  id: number;
  player: number;
  score: number;
  perfect: number;
  good: number;
  bad: number;
  miss: number;
  max_combo: number;
  accuracy: number; // 例如 1.0 代表 100%
  full_combo: boolean;
  std: number;
  std_score: number;
}
```

针对 SSE（`GET /rooms/listen`）的事件定义：

```typescript
// === SSE 房间监听事件 (SSE) ===

// 房间状态更新事件（可能全量也可能包含了某些特定字段修改）
// 建立连接时立刻收到 `create_room` 代表初始化全量数据
export interface SSEEventUpdateRoom {
  room: string;
  data: Partial<RoomData>; // 数据更新（如果不存在，前台可理解为创建房间）
}

// 玩家加入房间或离开房间事件
export interface SSEEventJoinOrLeaveRoom {
  room: string;
  user: number;
}

// 包含了新的对局结算
export interface SSEEventNewRound {
  room: string;
  round: RoundData;
}
```

#### `GET /chart/{id}`

**说明**：获取指定 `id` 谱面的二进制数据，供 `monitor-client` 解码使用。

**响应格式**：`application/octet-stream`。

#### `GET /rooms/info`

**说明**：获取当前所有活跃房间的列表。

**响应格式**：`application/json`，格式为 `RoomListResponse`。

#### `GET /rooms/info/{id}`

**说明**：获取指定 `id` 房间的详细信息。

**响应格式**：`application/json`，格式为 `RoomInfoResponse`。

#### `GET /rooms/user/{id}`

**说明**：查询指定用户（ID）当前所在的房间。

**响应格式**：`application/json`，格式为 `RoomInfoResponse` (如果不在任何房间中则为 `null`)。

#### `GET /rooms/listen`

**说明**：用于监听房间生命周期事件的 Server-Sent Events (SSE) 流。

**响应格式**：`text/event-stream`。

包含的事件类型：

- `update_room`: 发送 `SSEEventUpdateRoom` 结构的 JSON 数据。若房间 ID 不存在，那么代表创建新房间。当建立 SSE 连接时，服务端立刻发送若干 `update_room` 事件，表示当前所有房间的状态。
- `join_room`: 发送 `SSEEventJoinOrLeaveRoom` 结构的 JSON 数据。
- `leave_room`: 发送 `SSEEventJoinOrLeaveRoom` 结构的 JSON 数据。
- `new_round`: 发送 `SSEEventNewRound` 结构的 JSON 数据。

#### `POST /auth/login`

**说明**：代理到官方 Phira 认证接口的登录端点。成功后返回一个 JWT Token，前端需要保存该 Token 用于后续的认证请求。

**请求格式**：`application/json`，格式为 `LoginRequest`。

**响应格式**：`application/json`，格式为 `LoginResponse`。

#### `GET /auth/me`

**说明**：获取当前 JWT Token 对应的用户资料数据（在 Phira 原生数据的缓存）。需要在请求头中携带 `Authorization: Bearer <token>`。

**响应格式**：`application/json`，格式为 `ProfileResponse`。

---

## 开发指南

在本地开发本项目，请确保已安装 **Rust**、**Node.js (v18+)** 和 **wasm-pack**。

1. **编译 WASM 客户端：**

```bash
cd monitor-client
wasm-pack build --out-dir ../web/pkg --target web
```

2. **运行前端 (Vue)：**

```bash
cd web
npm install
npm run dev
```

3. **运行代理后端：**

_(注意先设置好本地开发用的 secret key)_

```bash
export HSN_SECRET_KEY=dev_secret_local
cargo run --bin monitor-proxy -- --debug
```

---

## 生产部署指南

部署 HSN Phira Proxy 需要编译静态的 WebAssembly/Vue 产物，并确保 Rust 服务器的安全运行。

### 前置要求

- 构建工具：`rustc`、`cargo`、`npm`、`wasm-pack`。
- Web 服务器（例如 Nginx 或 Caddy），用于托管静态网页并反向代理。

### 1. 编译 WASM 引擎

**此步骤必须最先执行**，因为 Vue 的构建依赖于输出到 `pkg/` 文件夹中的 WASM 模块。

```bash
cd monitor-client
wasm-pack build --target web --out-dir ../web/pkg --release
```

### 2. 编译静态 Web 前端

将 Vue 3 应用编译为标准的 HTML/JS 静态文件。

#### 环境变量配置 (.env)

在 `web` 目录下进行前端环境变量的配置。对于生产环境，你可以在构建前创建或修改 `.env.production` 文件：
如果你将前后端分离部署（API 后端并不和网页托管在同一个域名下），你需要指定前端访问代理后端的 API 根 URL：

```env
# 示例：代理后端的外部访问地址
VITE_API_BASE=https://api.yourdomain.com
```

_注：如果不配置或值为空字符串 `""`，前端会自动将请求发送至当前网页所在的同源相对路径（这非常适合使用 Nginx 统一进行反向代理的情况）。_

```bash
cd web
npm ci
npm run build
```

编译后优化过的前端文件将被输出到 `web/dist`。

### 3. 编译并运行 API 代理后端

使用 release 模式原生编译 Rust 二进制文件以获得最强性能。

```bash
cargo build --release --bin monitor-proxy
```

#### 启动选项指南

`monitor-proxy` 支持以下命令行参数，可通过 `--help` 查看：

- `--debug`: 开启调试模式。开启后 CORS 安全策略将被直接放宽。
- `--port <PORT>`: 服务器监听的端口（默认为 `3080`）。
- `--cache-dir <DIR>`: 谱面在硬盘上的缓存下载目录（默认存放在 `~/.cache/hsn-phira`）。
- `--api-base <URL>`: 指向 Phira 官方 API 的抓取地址（默认为 `https://phira.5wyxi.com`）。
- `--mp-server <ADDR>`: Phira 多人游戏服务器地址，用于获取房间信息（默认为 `localhost:12346`）。
- `--allowed-origin <ORIGIN>`: **在生产环境中必需**。设置明确的跨域资源共享（CORS）允许来源域名（例如 `https://monitor.example.com`）。如果不设置此项配置，则程序无法启动（除非你开启了 `--debug`）。

#### 环境变量

Rust 服务器还需要通过一个 Secret Key 来确保生成用户 token 时的加密安全，以及用于和 phira-mp 服务器沟通时的鉴权。在启动进程前**必须**定义它，**并且要和 phira-mp 设置的相同**。

```bash
export HSN_SECRET_KEY=$(openssl rand -hex 32)
```

启动服务器（推荐使用 systemd 或 PM2 等守护进程工具来后台统一管理，并传入生产参数）：

```bash
./target/release/monitor-proxy --port 8080 --allowed-origin https://monitor.example.com
```

### 4. 反向代理配置（以 Nginx 为例）

配置站点，使得 Web 服务器能够高效地托管 Vue 静态包，同时将 REST API、SSE 和 WebSocket 流量正确代理到后端的 Rust 服务器。

```nginx
server {
    listen 80;
    server_name monitor.example.com;

    # 托管 Vue 3 静态产物
    location / {
        root /path/to/hsn-phira/web/dist;
        try_files $uri $uri/ /index.html;
    }

    # 将 REST API、SSE 流 和 WebSocket 请求统一代理给 Rust 服务器
    location /api/ {
        proxy_pass http://127.0.0.1:8080/; # 请根据实际配置的 PORT 调整
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;

        # 针对 WebSocket 的 Upgrade 请求头配置
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "Upgrade";

        # 对于 SSE 流（例如 /rooms/listen），必须禁用缓冲机制以防止断连/延迟
        proxy_buffering off;
        proxy_read_timeout 86400; # 防止长连接掉线
    }
}
```
