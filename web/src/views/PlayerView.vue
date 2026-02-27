<template>
  <div class="play-page">
    <!-- Left Sidebar: Controls -->
    <aside class="sidebar">
      <n-card
        class="sidebar-card"
        content-style="display:flex;flex-direction:column;padding:1.25rem;"
      >
        <!-- Status Section -->
        <section class="section">
          <n-text depth="3" class="section-title">Status</n-text>
          <n-text depth="2" style="font-size: 0.82rem">{{ statusText }}</n-text>
          <n-text depth="3" style="font-size: 0.82rem">{{ wasmStatus }}</n-text>
        </section>

        <n-divider />

        <!-- Chart Loader Section -->
        <section class="section">
          <n-text depth="3" class="section-title">Chart Loader</n-text>
          <div class="input-group">
            <n-input
              v-model:value="chartId"
              placeholder="Chart ID (e.g. 1234)"
              style="flex: 1"
              @keydown.enter="loadChart"
            />
            <n-button type="primary" :loading="isLoading" @click="loadChart">
              Load
            </n-button>
          </div>
          <div class="parse-result" :class="parseClass">{{ parseResult }}</div>
          <n-text depth="3" italic style="font-size: 0.7rem">
            Tip: Use ID "test" for a local test chart.
          </n-text>
        </section>

        <n-divider />

        <!-- Playback Section -->
        <section class="section">
          <n-text depth="3" class="section-title">Playback</n-text>
          <div class="input-group">
            <n-button
              :type="isPaused ? 'success' : 'error'"
              style="flex: 1"
              @click="togglePlay"
            >
              {{ isPaused ? "Play" : "Pause" }}
            </n-button>
            <n-button
              :type="isAutoplay ? 'primary' : 'default'"
              @click="toggleAutoplay"
            >
              Autoplay: {{ isAutoplay ? "ON" : "OFF" }}
            </n-button>
          </div>
        </section>

        <!-- Chart Info (shown when loaded) -->
        <template v-if="chartInfo">
          <n-divider />
          <section class="section">
            <n-text depth="3" class="section-title">Chart Info</n-text>
            <n-descriptions label-placement="left" :column="1" size="small">
              <n-descriptions-item label="Song">
                {{ chartInfo.name }}
              </n-descriptions-item>
              <n-descriptions-item label="Composer">
                {{ chartInfo.composer }}
              </n-descriptions-item>
              <n-descriptions-item label="Charter">
                {{ chartInfo.charter }}
              </n-descriptions-item>
              <n-descriptions-item label="Level">
                {{ chartInfo.level }}
              </n-descriptions-item>
            </n-descriptions>
            <div class="stats">
              <n-statistic label="Difficulty" tabular-nums>
                <template #default>{{
                  chartInfo.difficulty.toFixed(1)
                }}</template>
              </n-statistic>
              <n-statistic label="Offset" tabular-nums>
                <template #default>{{ chartInfo.offset.toFixed(3) }}</template>
              </n-statistic>
              <n-statistic label="Format" tabular-nums>
                <template #default>{{
                  (chartInfo.format || "unknown").toUpperCase()
                }}</template>
              </n-statistic>
            </div>
          </section>
        </template>
      </n-card>
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
import {
  NCard,
  NInput,
  NButton,
  NText,
  NDivider,
  NDescriptions,
  NDescriptionsItem,
  NStatistic,
} from "naive-ui";
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
.parse-result.success {
  color: #4ade80;
}
.parse-result.error {
  color: #f87171;
}
.parse-result.loading {
  color: #facc15;
}

/* ── Stats ─────────────────────────────────────────────────────────── */
.stats {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 0.4rem;
  margin-top: 0.25rem;
}
.stats :deep(.n-statistic) {
  text-align: center;
  padding: 0.35rem;
  background: rgba(255, 255, 255, 0.04);
  border-radius: 6px;
}
.stats :deep(.n-statistic-value) {
  font-size: 0.9rem !important;
}
.stats :deep(.n-statistic .n-statistic-value__content) {
  color: #3a7bd5;
  font-weight: bold;
}
.stats :deep(.n-statistic-label) {
  font-size: 0.6rem !important;
  color: #64748b;
  margin-top: 2px;
}

/* ── Input group ──────────────────────────────────────────────────── */
.input-group {
  display: flex;
  gap: 0.4rem;
  align-items: center;
}

/* ── Naive UI descriptions compact ────────────────────────────────── */
:deep(.n-descriptions) {
  font-size: 0.82rem;
}
</style>
