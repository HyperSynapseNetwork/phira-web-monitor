<template>
  <div class="play-page">
    <canvas ref="canvasRef" id="gl-canvas" class="gl-canvas"></canvas>

    <div class="sidebar">
      <!-- Status Card -->
      <div class="card">
        <h2>Status</h2>
        <div class="status-line">{{ statusText }}</div>
        <div class="status-line wasm-status">{{ wasmStatus }}</div>
      </div>

      <!-- Chart Loader Card -->
      <div class="card">
        <h2>Chart Loader</h2>
        <div class="input-group">
          <input
            type="text"
            v-model="chartId"
            placeholder="Chart ID (e.g. 1234)"
            @keydown.enter="loadChart"
          />
          <button @click="loadChart" :disabled="isLoading">Load</button>
        </div>
        <div class="input-group">
          <button
            class="btn-play"
            :class="{ paused: !isPaused }"
            @click="togglePlay"
          >
            {{ isPaused ? "Play" : "Pause" }}
          </button>
          <button
            class="btn-autoplay"
            :class="{ off: !isAutoplay }"
            @click="toggleAutoplay"
          >
            AP: {{ isAutoplay ? "ON" : "OFF" }}
          </button>
        </div>

        <div class="parse-result" :class="parseClass">{{ parseResult }}</div>

        <!-- Chart Info -->
        <div v-if="chartInfo" class="chart-details">
          <div class="detail-row">
            <span class="label">Song:</span><span>{{ chartInfo.name }}</span>
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
            <span class="label">Level:</span><span>{{ chartInfo.level }}</span>
          </div>
        </div>

        <div v-if="chartInfo" class="stats">
          <div class="stat">
            <div class="stat-value">{{ chartInfo.difficulty.toFixed(1) }}</div>
            <div class="stat-label">Difficulty</div>
          </div>
          <div class="stat">
            <div class="stat-value">{{ chartInfo.offset.toFixed(3) }}</div>
            <div class="stat-label">Offset</div>
          </div>
          <div class="stat">
            <div class="stat-value">
              {{ (chartInfo.format || "unknown").toUpperCase() }}
            </div>
            <div class="stat-label">Format</div>
          </div>
        </div>

        <p class="tip">Tip: Use ID "test" for a local test chart.</p>
      </div>
    </div>
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
  if (canvas) {
    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;
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
  position: relative;
  overflow: hidden;
}

.gl-canvas {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  z-index: 0;
}

.sidebar {
  position: absolute;
  top: 0;
  right: 0;
  padding: 1.25rem;
  z-index: 10;
  pointer-events: none;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
}
.sidebar .card {
  pointer-events: auto;
}

.wasm-status {
  margin-top: 4px;
  color: #666;
}
.status-line {
  font-size: 0.85rem;
}

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

.parse-result {
  font-family: "Monaco", "Consolas", monospace;
  font-size: 0.78rem;
  white-space: pre-wrap;
  word-break: break-all;
  color: #e2e8f0;
  min-height: 40px;
  max-height: 120px;
  overflow-y: auto;
  padding: 0.6rem;
  background: rgba(0, 0, 0, 0.3);
  border-radius: 6px;
  margin-bottom: 0.5rem;
}

.chart-details {
  margin-top: 0.75rem;
  font-size: 0.82rem;
  border-top: 1px solid rgba(255, 255, 255, 0.1);
  padding-top: 0.5rem;
}
.detail-row {
  display: flex;
  justify-content: space-between;
  margin-bottom: 3px;
}
.detail-row .label {
  color: #94a3b8;
}

.stats {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 0.4rem;
  margin-top: 0.5rem;
}
.stat {
  text-align: center;
  padding: 0.4rem;
  background: rgba(255, 255, 255, 0.05);
  border-radius: 6px;
}
.stat-value {
  font-size: 1rem;
  font-weight: bold;
  color: #3a7bd5;
}
.stat-label {
  font-size: 0.6rem;
  color: #94a3b8;
  margin-top: 2px;
}
.tip {
  font-size: 0.72rem;
  color: #555;
  margin-top: 0.5rem;
}
</style>
