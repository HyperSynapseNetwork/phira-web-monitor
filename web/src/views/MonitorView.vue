<template>
  <div class="monitor-page">
    <!-- Left Sidebar: Controls -->
    <aside class="sidebar">
      <div class="card sidebar-card">
        <!-- Auth Section -->
        <section class="section">
          <h3>Authentication</h3>
          <div v-if="user" class="auth-info">
            <div class="user-row">
              <span class="user-name">{{ user.username }}</span>
              <span class="user-id-tag">#{{ user.phira_id }}</span>
            </div>
            <div class="user-rks">RKS {{ user.phira_rks.toFixed(2) }}</div>
          </div>
          <template v-else>
            <input
              type="text"
              v-model="email"
              placeholder="Email"
              class="auth-input"
            />
            <div class="input-group">
              <input
                type="password"
                v-model="password"
                placeholder="Password"
                @keydown.enter="login"
              />
              <button @click="login" :disabled="loggingIn">
                {{ loggingIn ? "..." : "Login" }}
              </button>
            </div>
            <div v-if="authError" class="field-error">{{ authError }}</div>
          </template>
        </section>

        <hr class="divider" />

        <!-- Connection Section -->
        <section class="section">
          <h3>Connection</h3>
          <div class="status-dot" :class="wsState">
            <span class="dot"></span>
            {{ wsLabel }}
          </div>
          <div class="input-group">
            <button
              @click="connect"
              :disabled="!user || wsState === 'connected'"
              style="flex: 1"
            >
              Connect
            </button>
            <button
              class="btn-danger"
              @click="disconnect"
              :disabled="wsState !== 'connected'"
            >
              ×
            </button>
          </div>
        </section>

        <hr class="divider" />

        <!-- Room Section -->
        <section class="section">
          <h3>Room</h3>
          <div class="input-group">
            <input type="text" v-model="roomId" placeholder="Room ID" />
            <button @click="joinRoom" :disabled="wsState !== 'connected'">
              Join
            </button>
            <button
              class="btn-secondary"
              @click="leaveRoom"
              :disabled="wsState !== 'connected'"
            >
              Leave
            </button>
          </div>
        </section>

        <hr class="divider" />

        <!-- Scenes Section -->
        <section class="section">
          <h3>Scenes</h3>
          <div class="input-group">
            <select
              v-model.number="sceneUserId"
              :disabled="!monitor || roomUsers.length === 0"
            >
              <option :value="null" disabled>Select player…</option>
              <option
                v-for="u in roomUsers"
                :key="u.id"
                :value="u.id"
                :disabled="scenes.some((s) => s.userId === u.id)"
              >
                {{ u.name }} (#{{ u.id }})
                <template v-if="scenes.some((s) => s.userId === u.id)">
                  ✓</template
                >
              </option>
            </select>
            <button
              @click="addScene"
              :disabled="!monitor || sceneUserId == null"
            >
              Add
            </button>
          </div>
          <div class="scene-tags" v-if="scenes.length > 0">
            <span v-for="s in scenes" :key="s.userId" class="scene-tag">
              #{{ s.userId }}
              <button class="tag-close" @click="removeSceneById(s.userId)">
                ×
              </button>
            </span>
          </div>
          <div v-else class="field-hint">No active scenes</div>
        </section>
      </div>
    </aside>

    <!-- Right: Scenes + Log -->
    <main class="main-area">
      <!-- Scene Grid -->
      <div class="scene-area" v-if="scenes.length > 0">
        <div v-for="scene in scenes" :key="scene.userId" class="scene-cell">
          <div class="scene-header">
            <span class="scene-label">Player #{{ scene.userId }}</span>
            <button
              class="btn-scene-close"
              @click="removeSceneById(scene.userId)"
            >
              ×
            </button>
          </div>
          <canvas
            :id="scene.canvasId"
            :ref="
              (el) => onCanvasRef(scene.userId, el as HTMLCanvasElement | null)
            "
            class="scene-canvas"
          ></canvas>
        </div>
      </div>
      <div class="scene-area scene-empty" v-else>
        <div class="scene-empty-text">
          No scenes. Join a room and add player scenes to start monitoring.
        </div>
      </div>

      <!-- Event Log -->
      <div class="card log-card">
        <h3>
          Event Log
          <button class="btn-clear" @click="clearLog">Clear</button>
        </h3>
        <div class="event-log" ref="logRef">
          <div
            v-for="(entry, i) in eventLog"
            :key="i"
            class="log-entry"
            :class="entry.level"
          >
            <span class="log-time">{{ entry.time }}</span>
            <span class="log-msg">{{ entry.message }}</span>
          </div>
          <div v-if="eventLog.length === 0" class="log-empty">
            No events yet.
          </div>
        </div>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, nextTick, onMounted, onUnmounted } from "vue";
import wasmInit, { GameMonitor } from "monitor-client";

const API_BASE = import.meta.env.VITE_API_BASE || "";

interface LogEntry {
  time: string;
  message: string;
  level: "info" | "warn" | "error" | "event";
}

interface UserInfo {
  username: string;
  phira_id: number;
  phira_rks: number;
  phira_avatar: string | null;
}

interface RoomUser {
  id: number;
  name: string;
  monitor: boolean;
}

interface SceneEntry {
  userId: number;
  canvasId: string;
  isCreated: boolean;
}

function wsBaseFromApi(apiBase: string): string {
  if (!apiBase)
    return `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}`;
  return apiBase.replace(/^http/, "ws");
}

const roomId = ref("");
const wsState = ref<"disconnected" | "connecting" | "connected">(
  "disconnected",
);
const wsLabel = ref("Disconnected");
const eventLog = ref<LogEntry[]>([]);
const logRef = ref<HTMLDivElement | null>(null);

// Auth state
const email = ref("");
const password = ref("");
const loggingIn = ref(false);
const authError = ref("");
const user = ref<UserInfo | null>(null);

// Scene state
const scenes = ref<SceneEntry[]>([]);
const sceneUserId = ref<number | null>(null);
const roomUsers = ref<RoomUser[]>([]);

let monitor: GameMonitor | null = null;
let tickRaf = 0;
let wasmReady = false;
let sceneCounter = 0;
const defaultFileMap = ref<Record<string, Uint8Array> | null>(null);

const SCENE_WIDTH = 480;
const SCENE_HEIGHT = 320;

function log(message: string, level: LogEntry["level"] = "info") {
  const now = new Date();
  const time = `${now.getHours().toString().padStart(2, "0")}:${now.getMinutes().toString().padStart(2, "0")}:${now.getSeconds().toString().padStart(2, "0")}`;
  eventLog.value.push({ time, message, level });
  if (eventLog.value.length > 500)
    eventLog.value.splice(0, eventLog.value.length - 500);
  nextTick(() => {
    if (logRef.value) logRef.value.scrollTop = logRef.value.scrollHeight;
  });
}

function clearLog() {
  eventLog.value = [];
}

// ── Room user tracking (parsed from GameMonitor console messages) ────

function handleGameMonitorMessage(msg: string) {
  // "  user: NAME (id=ID), monitor=BOOL"  — from join response
  const userLine = msg.match(
    /^\s+user:\s+(.+?)\s+\(id=(\d+)\),\s*monitor=(\w+)/,
  );
  if (userLine) {
    const [, name, id, mon] = userLine;
    const uid = parseInt(id);
    if (!roomUsers.value.some((u) => u.id === uid)) {
      roomUsers.value.push({ id: uid, name, monitor: mon === "true" });
    }
    return;
  }

  // "GameMonitor: user joined: NAME (id=ID), monitor=BOOL"
  const joined = msg.match(
    /user joined:\s+(.+?)\s+\(id=(\d+)\),\s*monitor=(\w+)/,
  );
  if (joined) {
    const [, name, id, mon] = joined;
    const uid = parseInt(id);
    if (!roomUsers.value.some((u) => u.id === uid)) {
      roomUsers.value.push({ id: uid, name, monitor: mon === "true" });
    }
    return;
  }

  // "GameMonitor: user left: id=ID"
  const left = msg.match(/user left:\s+id=(\d+)/);
  if (left) {
    const uid = parseInt(left[1]);
    roomUsers.value = roomUsers.value.filter((u) => u.id !== uid);
    return;
  }

  // "GameMonitor: joined room, N users"  — clear user list, will be repopulated
  if (msg.includes("joined room,")) {
    roomUsers.value = [];
    return;
  }

  // "GameMonitor: leave result"
  if (msg.includes("leave result")) {
    roomUsers.value = [];
    return;
  }
}

// ── Scene management ────────────────────────────────────────────────

function onCanvasRef(userId: number, el: HTMLCanvasElement | null) {
  if (!el || !monitor) return;
  const scene = scenes.value.find((s) => s.userId === userId);
  if (!scene || scene.isCreated) return;

  scene.isCreated = true;

  el.width = SCENE_WIDTH;
  el.height = SCENE_HEIGHT;

  nextTick(async () => {
    try {
      if (monitor) {
        monitor.create_scene(userId, scene.canvasId);
        if (defaultFileMap.value) {
          await monitor.load_scene_resource_pack(userId, defaultFileMap.value);
        }
        log(`Scene created for player #${userId}`, "event");
      }
    } catch (e) {
      scene.isCreated = false;
      log(`Failed to create scene for #${userId}: ${e}`, "error");
    }
  });
}

function addScene() {
  if (!monitor || sceneUserId.value == null) return;
  const uid = sceneUserId.value;
  if (scenes.value.some((s) => s.userId === uid)) {
    log(`Scene for #${uid} already exists`, "warn");
    return;
  }

  // Browsers enforce strict autoplay policies.
  // We must explicitly resume the explicit audio contexts attached to GameScenes
  // during a direct user gesture (like this button click)!
  try {
    monitor.resume_audio();
  } catch (e) {
    console.warn("Audio Context resume suppressed", e);
  }

  const canvasId = `scene-canvas-${uid}-${sceneCounter++}`;
  scenes.value.push({ userId: uid, canvasId, isCreated: false });
  log(`Adding scene for player #${uid}`, "info");
}

function removeSceneById(userId: number) {
  const idx = scenes.value.findIndex((s) => s.userId === userId);
  if (idx === -1) return;
  scenes.value.splice(idx, 1);
  if (monitor) {
    try {
      monitor.destroy_scene(userId);
      log(`Scene destroyed for player #${userId}`, "event");
    } catch (_) {}
  }
}

function clearAllScenes() {
  for (const scene of scenes.value) {
    if (monitor) {
      try {
        monitor.destroy_scene(scene.userId);
      } catch (_) {}
    }
  }
  scenes.value = [];
}

// ── Auth ──────────────────────────────────────────────────────────────
async function checkAuth() {
  try {
    const resp = await fetch(`${API_BASE}/auth/me`, { credentials: "include" });
    if (resp.ok) {
      user.value = await resp.json();
      log(`Authenticated as ${user.value!.username}`, "event");
    }
  } catch (_) {
    /* not logged in */
  }
}

async function login() {
  authError.value = "";
  loggingIn.value = true;
  try {
    const resp = await fetch(`${API_BASE}/auth/login`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      body: JSON.stringify({ email: email.value, password: password.value }),
    });
    if (!resp.ok) {
      const data = await resp.json().catch(() => ({}));
      authError.value = data.error || "Login failed";
      return;
    }
    await checkAuth();
    password.value = "";
  } catch (e) {
    authError.value = `Network error: ${e}`;
  } finally {
    loggingIn.value = false;
  }
}

// ── WebSocket ─────────────────────────────────────────────────────────
async function connect() {
  if (!user.value) return;
  if (!wasmReady) {
    log("Initializing WASM...", "warn");
    await wasmInit();
    wasmReady = true;
    log("WASM initialized");
  }
  if (monitor) {
    try {
      monitor.close();
    } catch (_) {}
    monitor = null;
  }
  clearAllScenes();
  roomUsers.value = [];

  const wsUrl = `${wsBaseFromApi(API_BASE)}/ws/live`;
  wsState.value = "connecting";
  wsLabel.value = "Connecting...";
  log(`Connecting to ${wsUrl}...`);

  try {
    monitor = new GameMonitor(wsUrl, API_BASE);
    wsState.value = "connected";
    wsLabel.value = "Connected";
    log("WebSocket opened", "event");
    startTicking();
  } catch (e) {
    wsState.value = "disconnected";
    wsLabel.value = "Connection Failed";
    log(`Connection failed: ${e}`, "error");
  }
}

function disconnect() {
  if (monitor) {
    try {
      monitor.close();
    } catch (_) {}
    monitor = null;
  }
  clearAllScenes();
  roomUsers.value = [];
  stopTicking();
  wsState.value = "disconnected";
  wsLabel.value = "Disconnected";
  log("Disconnected", "warn");
}

function joinRoom() {
  if (!monitor || !roomId.value) return;
  try {
    monitor.join_room(roomId.value);
    log(`Sent: JoinRoom(${roomId.value})`, "event");
  } catch (e) {
    log(`Join failed: ${e}`, "error");
  }
}

function leaveRoom() {
  if (!monitor) return;
  try {
    monitor.leave_room();
    clearAllScenes();
    roomUsers.value = [];
    log("Sent: LeaveRoom", "event");
  } catch (e) {
    log(`Leave failed: ${e}`, "error");
  }
}

function startTicking() {
  function tick(ts: number) {
    if (monitor) {
      try {
        monitor.tick(ts);
      } catch (_) {}
    }
    tickRaf = requestAnimationFrame(tick);
  }
  tickRaf = requestAnimationFrame(tick);
}
function stopTicking() {
  cancelAnimationFrame(tickRaf);
}

// Intercept console.log to capture "GameMonitor:" messages
const originalLog = console.log;
onMounted(async () => {
  const map: Record<string, Uint8Array> = {};
  for (const file of [
    "info.yml",
    "click.png",
    "click_mh.png",
    "drag.png",
    "drag_mh.png",
    "flick.png",
    "flick_mh.png",
    "hold.png",
    "hold_mh.png",
    "hit_fx.png",
    "click.ogg",
    "drag.ogg",
    "flick.ogg",
  ]) {
    try {
      const resp = await fetch(`/assets/respack/default/${file}`);
      if (resp.ok) {
        map[file] = new Uint8Array(await resp.arrayBuffer());
      }
    } catch (e) {
      console.warn(`Failed to preload ${file}:`, e);
    }
  }
  defaultFileMap.value = map;

  checkAuth();
  console.log = (...args: any[]) => {
    originalLog.apply(console, args);
    const msg = args
      .map((a) => (typeof a === "string" ? a : JSON.stringify(a)))
      .join(" ");
    if (msg.startsWith("GameMonitor") || msg.match(/^\s+user:/)) {
      log(msg, "event");
      handleGameMonitorMessage(msg);
    }
  };
});

onUnmounted(() => {
  console.log = originalLog;
  stopTicking();
  clearAllScenes();
  if (monitor) {
    try {
      monitor.close();
    } catch (_) {}
  }
});
</script>

<style scoped>
.monitor-page {
  flex: 1;
  overflow: hidden;
  padding: 1rem;
  display: grid;
  grid-template-columns: 320px 1fr;
  gap: 1rem;
  height: 100%;
}

/* ── Sidebar ──────────────────────────────────────────────────────── */
.sidebar {
  overflow-y: auto;
}
.sidebar-card {
  height: 100%;
  width: 100%;
  margin-bottom: 0;
  display: flex;
  flex-direction: column;
}

.section {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}
.section h3 {
  margin: 0;
  font-size: 0.7rem;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: #64748b;
  font-weight: 600;
}

.divider {
  border: none;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  margin: 0.6rem 0;
}

.input-group {
  display: flex;
  gap: 0.4rem;
}
.input-group input,
.input-group select {
  flex: 1;
  min-width: 0;
}

select {
  appearance: none;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  color: #e2e8f0;
  padding: 0.45rem 0.6rem;
  font-size: 0.82rem;
  font-family: inherit;
  cursor: pointer;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='6'%3E%3Cpath d='M0 0l5 6 5-6z' fill='%2364748b'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 0.5rem center;
  padding-right: 1.5rem;
}
select:focus {
  outline: none;
  border-color: rgba(58, 123, 213, 0.5);
}
select:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.field-error {
  color: #f87171;
  font-size: 0.78rem;
}
.field-hint {
  color: #475569;
  font-size: 0.78rem;
  font-style: italic;
}

.scene-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 0.3rem;
}
.scene-tag {
  display: inline-flex;
  align-items: center;
  gap: 0.2rem;
  background: rgba(58, 123, 213, 0.15);
  color: #7dd3fc;
  border-radius: 4px;
  padding: 0.15rem 0.4rem;
  font-size: 0.75rem;
  font-weight: 500;
}
.tag-close {
  background: transparent;
  border: none;
  color: #f87171;
  font-size: 0.85rem;
  cursor: pointer;
  padding: 0;
  line-height: 1;
}

/* ── Main area (right side) ───────────────────────────────────────── */
.main-area {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  overflow: hidden;
}

/* ── Scene Grid ───────────────────────────────────────────────────── */
.scene-area {
  flex: 1;
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem;
  align-content: flex-start;
  overflow-y: auto;
  min-height: 0;
}

.scene-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px dashed rgba(255, 255, 255, 0.08);
  border-radius: 10px;
}
.scene-empty-text {
  color: #475569;
  font-style: italic;
  font-size: 0.85rem;
}

.scene-cell {
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 8px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.scene-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.25rem 0.5rem;
  background: rgba(255, 255, 255, 0.04);
  border-bottom: 1px solid rgba(255, 255, 255, 0.04);
}
.scene-label {
  font-size: 0.72rem;
  font-weight: 600;
  color: #94a3b8;
}
.btn-scene-close {
  background: transparent;
  border: none;
  color: #f87171;
  font-size: 0.9rem;
  cursor: pointer;
  padding: 0 0.2rem;
  line-height: 1;
}

.scene-canvas {
  display: block;
  width: 480px;
  height: 320px;
  background: #0a0a0a;
}

/* ── Log Card ─────────────────────────────────────────────────────── */
.log-card {
  flex-shrink: 0;
  height: 320px;
  display: flex;
  flex-direction: column;
  width: 100%;
  margin-bottom: 0;
}
.log-card h3 {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin: 0 0 0.4rem;
  font-size: 0.7rem;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: #64748b;
  font-weight: 600;
}

.btn-clear {
  font-size: 0.65rem;
  padding: 0.15rem 0.4rem;
  background: rgba(255, 255, 255, 0.06);
  border-radius: 4px;
  text-transform: none;
  letter-spacing: 0;
  border: none;
  color: #94a3b8;
  cursor: pointer;
}

.event-log {
  flex: 1;
  overflow-y: auto;
  font-family: "Monaco", "Consolas", monospace;
  font-size: 0.72rem;
  background: rgba(0, 0, 0, 0.25);
  border-radius: 6px;
  padding: 0.4rem;
  min-height: 0;
}

.log-entry {
  padding: 1px 0;
  display: flex;
  gap: 0.4rem;
  line-height: 1.35;
}
.log-time {
  color: #334155;
  flex-shrink: 0;
}
.log-msg {
  word-break: break-all;
}

.log-entry.info .log-msg {
  color: #94a3b8;
}
.log-entry.warn .log-msg {
  color: #facc15;
}
.log-entry.error .log-msg {
  color: #f87171;
}
.log-entry.event .log-msg {
  color: #4ade80;
}
.log-empty {
  color: #334155;
  font-style: italic;
}

/* ── Auth ──────────────────────────────────────────────────────────── */
.auth-input {
  width: 100%;
  display: block;
}
.auth-info {
  font-size: 0.82rem;
}
.user-row {
  display: flex;
  align-items: baseline;
  gap: 0.35rem;
}
.user-name {
  font-weight: 600;
  color: #e2e8f0;
}
.user-id-tag {
  color: #475569;
  font-size: 0.75rem;
}
.user-rks {
  color: #3a7bd5;
  font-size: 0.78rem;
  margin-top: 1px;
}

/* ── Status ────────────────────────────────────────────────────────── */
.status-dot {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 0.82rem;
  color: #94a3b8;
}
.dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #475569;
  display: inline-block;
}
.connected .dot {
  background: #4ade80;
  box-shadow: 0 0 5px #4ade80;
}
.connecting .dot {
  background: #facc15;
  animation: pulse 1s infinite;
}
.disconnected .dot {
  background: #f87171;
}
@keyframes pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.4;
  }
}

/* ── Buttons ───────────────────────────────────────────────────────── */
.btn-danger {
  background: rgba(239, 68, 68, 0.25);
  color: #f87171;
  font-size: 0.95rem;
  padding: 0.4rem 0.5rem;
  line-height: 1;
}
.btn-secondary {
  background: linear-gradient(135deg, #475569, #334155);
}
</style>
