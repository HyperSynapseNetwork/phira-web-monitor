import init, { Monitor, MonitorView } from "monitor-client";

async function main() {
  console.log("Initializing Wasm...");
  await init();
  console.log("Wasm initialized.");

  const canvas = document.getElementById("gl-canvas") as HTMLCanvasElement;
  if (!canvas) {
    console.error("Canvas #gl-canvas not found!");
    return;
  }

  // Resize canvas to full screen
  function resize() {
    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;
  }
  window.addEventListener("resize", resize);
  resize();

  try {
    const monitor = new Monitor();
    console.log("Monitor instance created.");

    // Simulate monitoring user 1234
    const view = monitor.monitor(1234, "gl-canvas");
    console.log("MonitorView created.");

    const statusEl = document.getElementById("status");
    if (statusEl) statusEl.innerText = "Monitoring User 1234";
    const wasmStatusEl = document.getElementById("wasm-status");
    if (wasmStatusEl) wasmStatusEl.innerText = "Running";

    // Expose view for debugging
    (window as any).monitorView = view;

    // Load Resource Pack
    async function loadResourcePack() {
      console.log("Loading resource pack...");
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
      ];

      const fileMap: Record<string, Uint8Array> = {};

      try {
        await Promise.all(
          files.map(async (file) => {
            const resp = await fetch(`/assets/skin/${file}`);
            if (!resp.ok) {
              throw new Error(`Failed to fetch ${file}: ${resp.statusText}`);
            }
            const buf = await resp.arrayBuffer();
            fileMap[file] = new Uint8Array(buf);
          }),
        );

        console.log("Resource pack files fetched. Loading into WASM...");
        await view.load_resource_pack(fileMap);
        console.log("Resource pack loaded successfully.");
      } catch (e) {
        console.error("Failed to load resource pack:", e);
      }
    }

    // Start loading resource pack in background
    loadResourcePack();

    // Chart Loading Logic
    const loadBtn = document.getElementById("parse-btn");
    const chartIdInput = document.getElementById(
      "chart-id",
    ) as HTMLInputElement;

    let isLoading = false;

    if (loadBtn && chartIdInput) {
      loadBtn.onclick = async () => {
        const id = chartIdInput.value;
        if (!id) {
          alert("Please enter a Chart ID");
          return;
        }

        if (statusEl) statusEl.innerText = `Loading Chart ${id}...`;
        isLoading = true;

        try {
          const info = (await view.load_chart(id)) as any;
          if (statusEl) statusEl.innerText = `Chart ${id} Loaded`;
          console.log(`Chart ${id} loaded successfully.`, info);

          const parseResultEl = document.getElementById("parse-result");
          if (parseResultEl) {
            parseResultEl.innerText = `Successfully loaded chart: ${info.name}`;
            parseResultEl.className = "success";
          }

          const infoCard = document.getElementById("chart-info-details");
          const nameEl = document.getElementById("info-name");
          const composerEl = document.getElementById("info-composer");
          const charterEl = document.getElementById("info-charter");
          const levelEl = document.getElementById("info-level");

          if (infoCard) infoCard.style.display = "block";
          if (nameEl) nameEl.innerText = info.name;
          if (composerEl) composerEl.innerText = info.composer;
          if (charterEl) charterEl.innerText = info.charter;
          if (levelEl) levelEl.innerText = info.level;

          const statsCard = document.getElementById("stats");
          const difficultyEl = document.getElementById("stat-difficulty");
          const offsetEl = document.getElementById("stat-offset");
          const formatEl = document.getElementById("stat-format");

          if (statsCard) statsCard.style.display = "grid";
          if (difficultyEl) difficultyEl.innerText = info.difficulty.toFixed(1);
          if (offsetEl) offsetEl.innerText = info.offset.toFixed(3);
          if (formatEl)
            formatEl.innerText = (info.format || "unknown").toUpperCase();
        } catch (e) {
          console.error("Failed to load chart:", e);
          if (statusEl) statusEl.innerText = `Error loading chart ${id}`;

          const parseResultEl = document.getElementById("parse-result");
          if (parseResultEl) {
            parseResultEl.innerText = `Error: ${e}`;
            parseResultEl.className = "error";
          }
          alert(`Failed to load chart: ${e}`);
        } finally {
          isLoading = false;
          // Force a resize/render after loading to ensure valid state
          resize();
        }
      };
    }

    let errorCount = 0;

    function renderLoop() {
      if (isLoading) {
        requestAnimationFrame(renderLoop);
        return;
      }
      try {
        view.resize(canvas.width, canvas.height);
        view.render();
      } catch (e) {
        errorCount++;
        if (errorCount % 60 === 0) {
          console.error("Render loop error (throttled):", e);
        }
      }
      requestAnimationFrame(renderLoop);
    }

    requestAnimationFrame(renderLoop);
  } catch (e) {
    console.error("Error starting monitor:", e);
  }
}

main();
