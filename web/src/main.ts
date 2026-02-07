/**
 * Phira Web Monitor - Demo Entry Point
 *
 * This file demonstrates loading and using the WASM module,
 * with server-side chart parsing via the proxy.
 */

import init, {
  greet,
  decode_chart,
  WebGlRenderer,
} from "../../monitor-client/pkg/monitor_client.js";

// The proxy server handles /chart/:id requests
const PROXY_CHART_API = "/chart";

let renderer: WebGlRenderer | null = null;
let isPlaying = false;
let startTime = 0;

async function main() {
  // ... (keep existing DOM selection code) ...
  const resultEl = document.getElementById("result");
  const parseBtn = document.getElementById("parse-btn") as HTMLButtonElement;
  const chartIdInput = document.getElementById("chart-id") as HTMLInputElement;
  const parseResultEl = document.getElementById("parse-result");
  const statsEl = document.getElementById("stats");
  const wasmStatusEl = document.getElementById("wasm-status");

  if (
    !resultEl ||
    !parseBtn ||
    !chartIdInput ||
    !parseResultEl ||
    !statsEl ||
    !wasmStatusEl
  ) {
    console.error("Missing DOM elements");
    return;
  }

  try {
    // ... (keep init code) ...
    // Initialize WASM module
    wasmStatusEl.textContent = "Initializing WASM...";
    await init();
    wasmStatusEl.textContent = "WASM Ready";

    // Initialize WebGL Renderer
    try {
      renderer = new WebGlRenderer("gl-canvas");
      console.log("WebGL Renderer initialized");

      // Load textures
      const skinPath = "/assets/skin";
      await Promise.all([
        loadTexture(renderer, 1, `${skinPath}/click.png`),
        loadTexture(renderer, 2, `${skinPath}/drag.png`),
        loadTexture(renderer, 3, `${skinPath}/flick.png`),
        loadTexture(renderer, 4, `${skinPath}/hold.png`),
        loadTexture(renderer, 5, `${skinPath}/hit_fx.png`),
      ]);
      console.log("All textures loaded");

      // Start render loop
      requestAnimationFrame(renderLoop);

      // Handle resize
      window.addEventListener("resize", onResize);
      onResize(); // Initial resize
    } catch (e) {
      console.error("Failed to initialize WebGL renderer:", e);
      wasmStatusEl.textContent = `WebGL Error: ${e}`;
      wasmStatusEl.style.color = "#ff4444";
    }

    // ... (keep rest of main setup) ...
    // Test the greet function
    const message = greet("Phira Player");
    resultEl.textContent = message;
    resultEl.className = "";

    // Enable parse button
    parseBtn.disabled = false;

    // Setup parse button click handler
    parseBtn.addEventListener("click", async () => {
      const chartId = chartIdInput.value.trim();
      if (!chartId) {
        parseResultEl.textContent = "Please enter a chart ID";
        parseResultEl.className = "error";
        return;
      }

      await fetchAndDecodeChart(chartId, parseBtn, parseResultEl, statsEl);
    });

    // Allow Enter key to trigger parse
    chartIdInput.addEventListener("keypress", (e) => {
      if (e.key === "Enter" && !parseBtn.disabled) {
        parseBtn.click();
      }
    });
  } catch (error) {
    // ...
    resultEl.textContent = `Error: ${error}`;
    resultEl.className = "error";
    console.error("Failed to load WASM:", error);
  }
}

async function fetchAndDecodeChart(
  chartId: string,
  button: HTMLButtonElement,
  resultEl: HTMLElement,
  statsEl: HTMLElement,
) {
  button.disabled = true;
  resultEl.className = "loading";
  resultEl.textContent = `Processing chart ${chartId} on server...`;
  statsEl.style.display = "none";

  try {
    // Step 1: Fetch processed chart data from proxy (binary)
    console.time("fetch");
    const response = await fetch(`${PROXY_CHART_API}/${chartId}`);
    if (!response.ok) {
      // ...
      const text = await response.text();
      throw new Error(`Server error: ${response.status} - ${text}`);
    }

    const buffer = await response.arrayBuffer();
    const bytes = new Uint8Array(buffer);
    console.timeEnd("fetch");

    console.log(`Received ${bytes.length} bytes of chart data`);
    resultEl.textContent = `Decoding ${bytes.length} bytes...`;

    // Step 2: Decode using WASM (bincode -> struct)
    console.time("decode");
    const resultJson = decode_chart(bytes);
    console.timeEnd("decode");

    const parsed = JSON.parse(resultJson);

    if (parsed.success) {
      resultEl.className = "success";
      resultEl.textContent = `✓ Chart loaded successfully!\n\n(Server-side parsed & transferred as binary)`;

      // Load into renderer
      if (renderer) {
        renderer.load_chart(bytes);

        // Apply autoplay setting
        const autoplayCheck = document.getElementById(
          "autoplay-check",
        ) as HTMLInputElement;
        if (autoplayCheck) {
          renderer.set_autoplay(autoplayCheck.checked);

          // Add listener for changes
          autoplayCheck.onchange = () => {
            if (renderer) {
              renderer.set_autoplay(autoplayCheck.checked);
            }
          };
        }

        isPlaying = true;
        startTime = performance.now();
        console.log("Chart loaded into renderer, starting playback...");
      }

      // Update stats
      statsEl.style.display = "grid";
      (document.getElementById("stat-lines") as HTMLElement).textContent =
        parsed.lineCount.toString();
      (document.getElementById("stat-notes") as HTMLElement).textContent =
        parsed.noteCount.toString();
      (document.getElementById("stat-offset") as HTMLElement).textContent = (
        parsed.offset * 1000
      ).toFixed(0);

      console.log("Chart loaded:", parsed);
    } else {
      throw new Error(parsed.error || "Unknown decode error");
    }
  } catch (error) {
    // ...
    resultEl.className = "error";
    resultEl.textContent = `Error: ${error}`;
    console.error("Load error:", error);
    statsEl.style.display = "none";
  } finally {
    button.disabled = false;
  }
}

function onResize() {
  // ... (keep resizing logic) ...
  if (renderer) {
    const canvas = document.getElementById("gl-canvas") as HTMLCanvasElement;
    if (canvas) {
      // Update canvas size to match display size
      const displayWidth = window.innerWidth;
      const displayHeight = window.innerHeight;

      if (canvas.width !== displayWidth || canvas.height !== displayHeight) {
        canvas.width = displayWidth;
        canvas.height = displayHeight;
        renderer.resize(displayWidth, displayHeight);
      }
    }
  }
}

function renderLoop(time: number) {
  if (renderer) {
    let currentTime = 0;
    if (isPlaying) {
      currentTime = (performance.now() - startTime) / 1000.0;
    } else {
      currentTime = 0;
    }
    renderer.render(currentTime);
  }
  requestAnimationFrame(renderLoop);
}

main();

async function loadTexture(renderer: WebGlRenderer, id: number, url: string) {
  const img = new Image();
  img.crossOrigin = "Anonymous";
  img.src = url;

  await new Promise((resolve, reject) => {
    img.onload = resolve;
    img.onerror = reject;
  });

  const canvas = document.createElement("canvas");
  canvas.width = img.width;
  canvas.height = img.height;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("Could not get 2D context");

  // Draw directly (Top is Top)
  ctx.drawImage(img, 0, 0);

  const imageData = ctx.getImageData(0, 0, img.width, img.height);
  const bytes = new Uint8Array(imageData.data.buffer);

  renderer.load_texture(id, img.width, img.height, bytes);
  console.log(`Loaded texture ${id} from ${url} (${img.width}x${img.height})`);
}
