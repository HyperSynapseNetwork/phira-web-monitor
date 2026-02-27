<template>
  <div class="monitor-page">
    <!-- Left Sidebar: Controls -->
    <aside class="sidebar">
      <n-card
        class="sidebar-card"
        content-style="display:flex;flex-direction:column;padding:1.25rem;"
      >
        <!-- Auth Section -->
        <section class="section">
          <n-text depth="3" class="section-title">Authentication</n-text>
          <div v-if="user" class="auth-info">
            <div class="user-row">
              <n-text strong>{{ user.username }}</n-text>
              <n-text depth="3" style="font-size: 0.75rem"
                >#{{ user.phira_id }}</n-text
              >
            </div>
            <n-text type="info" style="font-size: 0.78rem">
              RKS {{ user.phira_rks.toFixed(2) }}
            </n-text>
          </div>
          <template v-else>
            <n-input
              v-model:value="email"
              placeholder="Email"
              style="margin-bottom: 0.4rem"
            />
            <div class="input-group">
              <n-input
                v-model:value="password"
                type="password"
                show-password-on="click"
                placeholder="Password"
                style="flex: 1"
                @keydown.enter="login"
              />
              <n-button type="primary" :loading="loggingIn" @click="login">
                Login
              </n-button>
            </div>
            <n-text v-if="authError" type="error" style="font-size: 0.78rem">
              {{ authError }}
            </n-text>
          </template>
        </section>

        <n-divider />

        <!-- Connection Section -->
        <section class="section">
          <n-text depth="3" class="section-title">Connection</n-text>
          <div class="status-dot" :class="wsState">
            <span class="dot"></span>
            {{ wsLabel }}
          </div>
          <div class="input-group">
            <n-button
              type="primary"
              style="flex: 1"
              :disabled="!user || wsState === 'connected'"
              @click="connect"
            >
              Connect
            </n-button>
            <n-button
              type="error"
              quaternary
              class="disconnect-btn"
              :disabled="wsState !== 'connected'"
              @click="disconnect"
            >
              ×
            </n-button>
          </div>
        </section>

        <n-divider />

        <!-- Room Section -->
        <section class="section">
          <n-text depth="3" class="section-title">Room</n-text>
          <div class="input-group">
            <n-input
              v-model:value="roomId"
              placeholder="Room ID"
              style="flex: 1"
            />
            <n-button
              type="primary"
              :disabled="wsState !== 'connected'"
              @click="joinRoom"
            >
              Join
            </n-button>
            <n-button :disabled="wsState !== 'connected'" @click="leaveRoom">
              Leave
            </n-button>
          </div>
        </section>

        <n-divider />

        <!-- Player Monitor Section -->
        <section class="section">
          <n-text depth="3" class="section-title">Monitor</n-text>
          <n-select
            v-model:value="selectedUserId"
            :options="playerOptions"
            placeholder="Select player…"
            :disabled="!monitor || roomUsers.length === 0"
            @update:value="selectPlayer"
          />
        </section>
      </n-card>
    </aside>

    <!-- Right: Scene + Log -->
    <main class="main-area">
      <!-- Single Scene Display -->
      <div class="scene-area">
        <div v-if="activeScene" class="scene-display">
          <div class="scene-header">
            <n-text depth="3" style="font-size: 0.72rem; font-weight: 600">
              {{
                roomUsers.find((u) => u.id === activeScene!.userId)?.name ||
                "Player"
              }}
              (#{{ activeScene!.userId }})
            </n-text>
          </div>
          <canvas
            :id="activeScene.canvasId"
            :ref="
              (el) => {
                if (activeScene)
                  onCanvasRef(
                    activeScene.userId,
                    el as HTMLCanvasElement | null,
                  );
              }
            "
            class="scene-canvas"
          ></canvas>
        </div>
        <div v-else class="scene-empty">
          <n-text depth="3" italic style="font-size: 0.85rem">
            Select a player from the sidebar to start monitoring.
          </n-text>
        </div>
      </div>

      <!-- Event Log -->
      <n-card
        class="log-card"
        content-style="display:flex;flex-direction:column;flex:1;min-height:0;padding:0.75rem;"
      >
        <div
          style="
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 0.4rem;
          "
        >
          <n-text depth="3" class="section-title" style="margin-bottom: 0"
            >Event Log</n-text
          >
          <n-button quaternary size="tiny" @click="clearLog">Clear</n-button>
        </div>
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
          <n-text
            v-if="eventLog.length === 0"
            depth="3"
            italic
            style="font-size: 0.72rem"
          >
            No events yet.
          </n-text>
        </div>
      </n-card>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, nextTick, onMounted, onUnmounted } from "vue";
import { NCard, NInput, NButton, NText, NDivider, NSelect } from "naive-ui";
import type { SelectOption } from "naive-ui";
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
const activeScene = ref<SceneEntry | null>(null);
const selectedUserId = ref<number | null>(null);
const roomUsers = ref<RoomUser[]>([]);

// Computed options for n-select
const playerOptions = computed<SelectOption[]>(() =>
  roomUsers.value.map((u) => ({
    label: `${u.name} (#${u.id})`,
    value: u.id,
  })),
);

let monitor: GameMonitor | null = null;
let monitorBusy = false;
let tickRaf = 0;
let wasmReady = false;
let sceneCounter = 0;
const defaultFileMap = ref<Record<string, Uint8Array> | null>(null);

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
    // If we were monitoring the user who left, clear
    if (selectedUserId.value === uid) {
      detachCurrentScene();
      selectedUserId.value = null;
    }
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

let pendingResize = false;
let resizeObserver: ResizeObserver | null = null;

function initResizeObserver() {
  if (!resizeObserver) {
    resizeObserver = new ResizeObserver(() => {
      applyResize();
    });
  }
  return resizeObserver;
}

function applyResize() {
  if (!activeScene.value || !monitor) return;
  const canvas = document.getElementById(
    activeScene.value.canvasId,
  ) as HTMLCanvasElement;
  if (!canvas) return;

  // Let CSS flexbox define the actual available layout bounds
  const targetW = canvas.clientWidth;
  const targetH = canvas.clientHeight;
  if (targetW === 0 || targetH === 0) return;

  if (canvas.width !== targetW || canvas.height !== targetH) {
    if (!monitorBusy) {
      try {
        monitor.resize_scene(activeScene.value.userId, targetW, targetH);
        canvas.width = targetW;
        canvas.height = targetH;
        pendingResize = false;
      } catch (e) {
        console.warn("Resize failed:", e);
      }
    } else {
      pendingResize = true;
    }
  }
}

function onCanvasRef(userId: number, el: HTMLCanvasElement | null) {
  if (!el || !monitor) return;
  const scene = activeScene.value;
  if (!scene || scene.userId !== userId || scene.isCreated) return;

  scene.isCreated = true;

  if (el.parentElement) {
    initResizeObserver().disconnect();
    initResizeObserver().observe(el.parentElement);
  }

  nextTick(async () => {
    try {
      if (monitor) {
        monitor.attach_canvas(userId, scene.canvasId);
        if (defaultFileMap.value) {
          monitorBusy = true;
          try {
            await monitor.load_scene_resource_pack(
              userId,
              defaultFileMap.value,
            );
          } finally {
            monitorBusy = false;
            if (pendingResize) applyResize();
          }
        } else {
          applyResize();
        }
        log(`Scene attached for player #${userId}`, "event");
      }
    } catch (e) {
      scene.isCreated = false;
      log(`Failed to attach scene for #${userId}: ${e}`, "error");
    }
  });
}

function selectPlayer(uid: number) {
  if (selectedUserId.value === uid && activeScene.value) return;

  // Detach previous scene
  detachCurrentScene();

  // Resume audio context on user gesture
  if (monitor) {
    try {
      monitor.resume_audio();
    } catch (e) {
      console.warn("Audio Context resume suppressed", e);
    }
  }

  selectedUserId.value = uid;
  const canvasId = `scene-canvas-${uid}-${sceneCounter++}`;
  activeScene.value = { userId: uid, canvasId, isCreated: false };
  log(`Monitoring player #${uid}`, "info");
}

function detachCurrentScene() {
  if (activeScene.value && monitor) {
    try {
      monitor.detach_canvas(activeScene.value.userId);
    } catch (_) {}
  }
  activeScene.value = null;
}

function clearAllScenes() {
  detachCurrentScene();
  selectedUserId.value = null;
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
    if (monitor && !monitorBusy) {
      // Poll connection status — more robust than parsing console.log
      if (!monitor.is_connected()) {
        handleDisconnect();
      } else {
        try {
          monitor.tick(ts);
        } catch (_) {}
      }
    }
    tickRaf = requestAnimationFrame(tick);
  }
  tickRaf = requestAnimationFrame(tick);
}
function stopTicking() {
  cancelAnimationFrame(tickRaf);
}

function handleDisconnect() {
  clearAllScenes();
  roomUsers.value = [];
  monitor = null;
  stopTicking();
  wsState.value = "disconnected";
  wsLabel.value = "Disconnected";
  log("WebSocket disconnected", "warn");
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
  grid-template-columns: clamp(240px, 20vw, 400px) 1fr;
  gap: 1rem;
  height: 100%;
}

/* ── Sidebar ──────────────────────────────────────────────────────── */
.sidebar {
  overflow-y: auto;
}
.sidebar-card {
  height: 100%;
}
.sidebar-card :deep(.n-card__content) {
  flex: 1;
}

.section {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}
.section-title {
  font-size: 0.7rem !important;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  font-weight: 600;
  margin-bottom: 0.1rem;
}

.auth-info {
  font-size: 0.82rem;
}
.user-row {
  display: flex;
  align-items: baseline;
  gap: 0.35rem;
}

/* ── Main area (right side) ───────────────────────────────────────── */
.main-area {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  overflow: hidden;
}

/* ── Scene ────────────────────────────────────────────────────────── */
.scene-area {
  flex: 1;
  display: flex;
  min-height: 0;
  overflow: hidden;
}

.scene-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px dashed rgba(255, 255, 255, 0.08);
  border-radius: 10px;
}

.scene-display {
  flex: 1;
  display: flex;
  flex-direction: column;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 8px;
  overflow: hidden;
  min-height: 0;
}

.scene-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.25rem 0.5rem;
  background: rgba(255, 255, 255, 0.04);
  border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  flex-shrink: 0;
}

.scene-canvas {
  display: block;
  width: 100%;
  flex: 1;
  min-height: 0;
  background: #0a0a0a;
  object-fit: contain;
}

/* ── Log Card ─────────────────────────────────────────────────────── */
.log-card {
  flex-shrink: 0;
  height: clamp(120px, 20vh, 300px);
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

/* ── Status Dot ───────────────────────────────────────────────────── */
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

/* ── Input group ──────────────────────────────────────────────────── */
.input-group {
  display: flex;
  gap: 0.4rem;
  align-items: center;
}

.disconnect-btn:enabled {
  background-color: rgba(248, 113, 113, 0.15) !important;
  opacity: 1 !important;
}
</style>
