<template>
  <div class="play-page">
    <!-- Left Sidebar: Controls -->
    <aside class="sidebar">
      <div class="card sidebar-card">
        <!-- Status Section -->
        <section class="section">
          <h3>Status</h3>
          <div class="status-line">{{ statusText }}</div>
          <div class="status-line wasm-status">{{ wasmStatus }}</div>
        </section>

        <hr class="divider" />

        <!-- Chart Loader Section -->
        <section class="section">
          <h3>Chart Loader</h3>
          <div class="input-group">
            <input
              type="text"
              v-model="chartId"
              placeholder="Chart ID (e.g. 1234)"
              @keydown.enter="loadChart"
            />
            <button @click="loadChart" :disabled="isLoading">Load</button>
          </div>
          <div class="parse-result" :class="parseClass">{{ parseResult }}</div>
          <p class="tip">Tip: Use ID "test" for a local test chart.</p>
        </section>

        <hr class="divider" />

        <!-- Playback Section -->
        <section class="section">
          <h3>Playback</h3>
          <div class="input-group">
            <button
              class="btn-play"
              :class="{ paused: !isPaused }"
              @click="togglePlay"
              style="flex: 1"
            >
              {{ isPaused ? "Play" : "Pause" }}
            </button>
            <button
              class="btn-autoplay"
              :class="{ off: !isAutoplay }"
              @click="toggleAutoplay"
            >
              Autoplay: {{ isAutoplay ? "ON" : "OFF" }}
            </button>
          </div>
        </section>

        <!-- Chart Info (shown when loaded) -->
        <template v-if="chartInfo">
          <hr class="divider" />
          <section class="section">
            <h3>Chart Info</h3>
            <div class="chart-details">
              <div class="detail-row">
                <span class="label">Song:</span
                ><span>{{ chartInfo.name }}</span>
              </div>
              <div class="detail-row">
                <span class="label">Composer:</span
                ><span>{{ chartInfo.composer }}</span>
              </div>
              <div class="detail-row">
                <span class="label">Charter:</span
                ><span>{{ chartInfo.charter }}</span>
              </div>
              <div class="detail-row">
                <span class="label">Level:</span
                ><span>{{ chartInfo.level }}</span>
              </div>
            </div>
            <div class="stats">
              <div class="stat">
                <div class="stat-value">
                  {{ chartInfo.difficulty.toFixed(1) }}
                </div>
                <div class="stat-label">Difficulty</div>
              </div>
              <div class="stat">
                <div class="stat-value">
                  {{ chartInfo.offset.toFixed(3) }}
                </div>
                <div class="stat-label">Offset</div>
              </div>
              <div class="stat">
                <div class="stat-value">
                  {{ (chartInfo.format || "unknown").toUpperCase() }}
                </div>
                <div class="stat-label">Format</div>
              </div>
            </div>
          </section>
        </template>
      </div>
    </aside>

    <!-- Right: Canvas -->
    <main class="main-area">
      <div class="canvas-area">
        <canvas ref="canvasRef" id="gl-canvas" class="gl-canvas"></canvas>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import init, { ChartPlayer } from "monitor-client";

const API_BASE = import.meta.env.VITE_API_BASE || "";

const canvasRef = ref<HTMLCanvasElement | null>(null);
const statusText = ref("Initializing...");
const wasmStatus = ref("Loading WASM...");
const chartId = ref("");
const isLoading = ref(false);
const isPaused = ref(true);
const isAutoplay = ref(true);
const parseResult = ref("Enter an ID to load.");
const parseClass = ref("");
const chartInfo = ref<any>(null);

let player: ChartPlayer | null = null;
let rafId = 0;

function resize() {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const rect = canvas.parentElement?.getBoundingClientRect();
  if (rect) {
    canvas.width = rect.width;
    canvas.height = rect.height;
  }
}

async function loadResourcePack() {
  if (!player) return;
  const files = [
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
  ];
  const fileMap: Record<string, Uint8Array> = {};
  await Promise.all(
    files.map(async (file) => {
      const resp = await fetch(`/assets/respack/default/${file}`);
      if (!resp.ok)
        throw new Error(`Failed to fetch ${file}: ${resp.statusText}`);
      fileMap[file] = new Uint8Array(await resp.arrayBuffer());
    }),
  );
  await player.load_resource_pack(fileMap);
  console.log("Resource pack loaded.");
}

async function loadChart() {
  if (!player || !chartId.value) return;
  isLoading.value = true;
  statusText.value = `Loading Chart ${chartId.value}...`;
  parseClass.value = "loading";
  parseResult.value = "Loading...";
  try {
    const info = (await player.load_chart(chartId.value)) as any;
    chartInfo.value = info;
    parseResult.value = `Loaded: ${info.name}`;
    parseClass.value = "success";
    statusText.value = `Chart ${chartId.value} loaded`;
    isPaused.value = true;
  } catch (e) {
    parseResult.value = `Error: ${e}`;
    parseClass.value = "error";
    statusText.value = `Error loading chart`;
  } finally {
    isLoading.value = false;
    resize();
  }
}

async function togglePlay() {
  if (!player) return;
  try {
    if (isPaused.value) {
      await player.resume();
      isPaused.value = false;
    } else {
      await player.pause();
      isPaused.value = true;
    }
  } catch (e) {
    console.error("Audio error:", e);
  }
}

function toggleAutoplay() {
  if (!player) return;
  isAutoplay.value = !isAutoplay.value;
  player.set_autoplay(isAutoplay.value);
}

let errorCount = 0;
function renderLoop() {
  if (!isLoading.value && player && canvasRef.value) {
    try {
      player.resize(canvasRef.value.width, canvasRef.value.height);
      player.render();
    } catch (e) {
      if (++errorCount % 60 === 0)
        console.error("Render error (throttled):", e);
    }
  }
  rafId = requestAnimationFrame(renderLoop);
}

onMounted(async () => {
  window.addEventListener("resize", resize);
  resize();
  await init();
  wasmStatus.value = "Running";
  const canvas = canvasRef.value;
  if (!canvas) return;
  player = new ChartPlayer("gl-canvas", API_BASE || undefined);
  statusText.value = "Active";
  (window as any).chartPlayer = player;
  loadResourcePack();
  rafId = requestAnimationFrame(renderLoop);
});

onUnmounted(() => {
  cancelAnimationFrame(rafId);
  window.removeEventListener("resize", resize);
});
</script>

<style scoped>
.play-page {
  flex: 1;
  overflow: hidden;
  padding: 1rem;
  display: grid;
  grid-template-columns: clamp(240px, 20vw, 400px) 1fr;
  gap: 1rem;
  height: 100%;
  color-scheme: dark;
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
  margin-bottom: 0;
}
.input-group input {
  flex: 1;
  min-width: 0;
}

input,
button {
  font-family: inherit;
  font-size: 0.85rem;
}

input {
  background: #0b1120;
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: #e2e8f0;
  padding: 0.5rem 0.6rem;
  border-radius: 6px;
  transition: all 0.2s;
}
input:focus {
  outline: none;
  border-color: rgba(58, 123, 213, 0.5);
  box-shadow: 0 0 0 2px rgba(58, 123, 213, 0.15);
}

/* ── Main area ────────────────────────────────────────────────────── */
.main-area {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.canvas-area {
  flex: 1;
  min-height: 0;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 8px;
  overflow: hidden;
}

.gl-canvas {
  display: block;
  width: 100%;
  height: 100%;
  background: #0a0a0a;
}

/* ── Status ────────────────────────────────────────────────────────── */
.wasm-status {
  color: #475569;
}
.status-line {
  font-size: 0.82rem;
  color: #94a3b8;
}

/* ── Buttons ──────────────────────────────────────────────────────── */
.btn-play {
  background: linear-gradient(135deg, #10b981, #059669);
}
.btn-play.paused {
  background: linear-gradient(135deg, #ef4444, #dc2626);
}
.btn-autoplay {
  background: linear-gradient(135deg, #8b5cf6, #6d28d9);
  font-size: 0.75rem;
}
.btn-autoplay.off {
  background: linear-gradient(135deg, #64748b, #475569);
}

/* ── Parse result ─────────────────────────────────────────────────── */
.parse-result {
  font-family: "Monaco", "Consolas", monospace;
  font-size: 0.75rem;
  white-space: pre-wrap;
  word-break: break-all;
  color: #e2e8f0;
  min-height: 32px;
  max-height: 80px;
  overflow-y: auto;
  padding: 0.4rem 0.5rem;
  background: rgba(0, 0, 0, 0.25);
  border-radius: 6px;
}

.tip {
  font-size: 0.7rem;
  color: #475569;
  font-style: italic;
}

/* ── Chart Info ────────────────────────────────────────────────────── */
.chart-details {
  font-size: 0.82rem;
}
.detail-row {
  display: flex;
  justify-content: space-between;
  margin-bottom: 3px;
}
.detail-row .label {
  color: #64748b;
}

.stats {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 0.4rem;
  margin-top: 0.25rem;
}
.stat {
  text-align: center;
  padding: 0.35rem;
  background: rgba(255, 255, 255, 0.04);
  border-radius: 6px;
}
.stat-value {
  font-size: 0.9rem;
  font-weight: bold;
  color: #3a7bd5;
}
.stat-label {
  font-size: 0.6rem;
  color: #64748b;
  margin-top: 2px;
}
</style>
