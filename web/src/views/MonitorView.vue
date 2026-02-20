<template>
  <div class="monitor-page">
    <div class="monitor-layout">
      <!-- Left: Auth, Connection & Room Controls -->
      <div class="panel">
        <!-- Auth Card -->
        <div class="card">
          <h2>Authentication</h2>
          <div v-if="user" class="auth-info">
            <div class="user-row">
              <span class="user-name">{{ user.username }}</span>
              <span class="user-id">#{{ user.phira_id }}</span>
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
            <div class="input-group" style="margin-top: 0.5rem">
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
            <div v-if="authError" class="auth-error">{{ authError }}</div>
          </template>
        </div>

        <!-- Connection Card -->
        <div class="card">
          <h2>WebSocket Connection</h2>
          <div class="status-dot" :class="wsState">
            <span class="dot"></span>
            {{ wsLabel }}
          </div>
          <div class="input-group" style="margin-top: 0.75rem">
            <button
              @click="connect"
              :disabled="!user || wsState === 'connected'"
              style="flex: 1"
            >
              Connect
            </button>
            <button
              class="btn-disconnect"
              @click="disconnect"
              :disabled="wsState !== 'connected'"
            >
              ×
            </button>
          </div>
        </div>

        <!-- Room Card -->
        <div class="card">
          <h2>Room</h2>
          <div class="input-group">
            <input type="text" v-model="roomId" placeholder="Room ID" />
            <button @click="joinRoom" :disabled="wsState !== 'connected'">
              Join
            </button>
            <button
              class="btn-leave"
              @click="leaveRoom"
              :disabled="wsState !== 'connected'"
            >
              Leave
            </button>
          </div>
        </div>
      </div>

      <!-- Right: Event Log -->
      <div class="panel log-panel">
        <div class="card log-card">
          <h2>
            Event Log
            <button class="btn-clear" @click="clearLog">Clear</button>
          </h2>
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
              No events yet. Log in and connect to start.
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, nextTick, onMounted, onUnmounted } from "vue";
import wasmInit, { Monitor } from "monitor-client";

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

let monitor: Monitor | null = null;
let tickRaf = 0;
let wasmReady = false;

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

  const wsUrl = `${wsBaseFromApi(API_BASE)}/ws/live`;
  wsState.value = "connecting";
  wsLabel.value = "Connecting...";
  log(`Connecting to ${wsUrl}...`);

  try {
    monitor = new Monitor(wsUrl);
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

// Intercept console.log to capture "Monitor:" messages
const originalLog = console.log;
onMounted(() => {
  checkAuth();
  console.log = (...args: any[]) => {
    originalLog.apply(console, args);
    const msg = args
      .map((a) => (typeof a === "string" ? a : JSON.stringify(a)))
      .join(" ");
    if (msg.startsWith("Monitor")) log(msg, "event");
  };
});

onUnmounted(() => {
  console.log = originalLog;
  stopTicking();
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
  padding: 1.25rem;
}

.monitor-layout {
  display: grid;
  grid-template-columns: 360px 1fr;
  gap: 1rem;
  height: 100%;
}

.panel {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.log-panel {
  min-width: 0;
}

.log-card {
  flex: 1;
  display: flex;
  flex-direction: column;
  width: 100%;
}
.log-card h2 {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.btn-clear {
  font-size: 0.7rem;
  padding: 0.2rem 0.5rem;
  background: rgba(255, 255, 255, 0.08);
  border-radius: 4px;
  text-transform: none;
  letter-spacing: 0;
}

.event-log {
  flex: 1;
  overflow-y: auto;
  font-family: "Monaco", "Consolas", monospace;
  font-size: 0.78rem;
  background: rgba(0, 0, 0, 0.3);
  border-radius: 6px;
  padding: 0.5rem;
  min-height: 200px;
  max-height: calc(100vh - 220px);
}

.log-entry {
  padding: 2px 0;
  display: flex;
  gap: 0.5rem;
  line-height: 1.4;
}
.log-time {
  color: #555;
  flex-shrink: 0;
}
.log-msg {
  word-break: break-all;
}

.log-entry.info .log-msg {
  color: #c9d1d9;
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
  color: #555;
  font-style: italic;
}

/* Auth */
.auth-input {
  width: 100%;
  display: block;
}
.auth-info {
  font-size: 0.85rem;
}
.user-row {
  display: flex;
  align-items: baseline;
  gap: 0.4rem;
}
.user-name {
  font-weight: 600;
  color: #fff;
}
.user-id {
  color: #64748b;
  font-size: 0.8rem;
}
.user-rks {
  color: #3a7bd5;
  font-size: 0.8rem;
  margin-top: 2px;
}
.auth-error {
  color: #f87171;
  font-size: 0.8rem;
  margin-top: 0.3rem;
}

/* WS Status */
.status-dot {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.85rem;
}
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #555;
  display: inline-block;
}
.connected .dot {
  background: #4ade80;
  box-shadow: 0 0 6px #4ade80;
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

.btn-disconnect {
  background: rgba(239, 68, 68, 0.3);
  color: #f87171;
  font-size: 1rem;
  padding: 0.5rem 0.6rem;
  line-height: 1;
}
.btn-leave {
  background: linear-gradient(135deg, #64748b, #475569);
}
</style>
