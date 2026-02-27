<template>
  <div class="app">
    <nav class="top-nav">
      <h1 class="brand">Phira Web Monitor</h1>
      <div class="nav-links">
        <button
          class="nav-link"
          :class="{ active: activeTab === 'play' }"
          @click="activeTab = 'play'"
        >
          Play
        </button>
        <button
          class="nav-link"
          :class="{ active: activeTab === 'monitor' }"
          @click="activeTab = 'monitor'"
        >
          Monitor
        </button>
      </div>
    </nav>
    <!-- v-show keeps both components alive so WebGL/WS state is preserved -->
    <PlayerView v-show="activeTab === 'play'" />
    <MonitorView v-show="activeTab === 'monitor'" />
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import PlayerView from "./views/PlayerView.vue";
import MonitorView from "./views/MonitorView.vue";

const activeTab = ref<"play" | "monitor">("monitor");
</script>

<style>
/* ── Reset & Global ────────────────────────────────────────────────── */
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  font-family:
    "Inter",
    -apple-system,
    BlinkMacSystemFont,
    "Segoe UI",
    Roboto,
    Oxygen,
    Ubuntu,
    Cantarell,
    "Open Sans",
    "Helvetica Neue",
    sans-serif;
  background: #0d1117;
  color: #c9d1d9;
  overflow: hidden;
  width: 100vw;
  height: 100vh;
}

.app {
  width: 100vw;
  height: 100vh;
  display: flex;
  flex-direction: column;
}

/* ── Top Navigation Bar ────────────────────────────────────────────── */
.top-nav {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 1.5rem;
  height: 52px;
  background: rgba(13, 17, 23, 0.95);
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  backdrop-filter: blur(12px);
  z-index: 100;
  flex-shrink: 0;
}

.brand {
  font-size: 1.1rem;
  font-weight: 700;
  background: linear-gradient(90deg, #00d2ff, #3a7bd5);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}

.nav-links {
  display: flex;
  gap: 0.25rem;
}

.nav-link {
  padding: 0.4rem 1rem;
  border-radius: 6px;
  text-decoration: none;
  font-size: 0.85rem;
  font-weight: 500;
  color: #8b949e;
  transition: all 0.15s;
  border: none;
  background: transparent;
  cursor: pointer;
}
.nav-link:hover {
  color: #c9d1d9;
  background: rgba(255, 255, 255, 0.06);
}
.nav-link.active {
  color: #fff;
  background: rgba(58, 123, 213, 0.25);
}

/* ── Shared Utilities ──────────────────────────────────────────────── */
.card {
  padding: 1.25rem;
  background: rgba(13, 17, 23, 0.85);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  margin-bottom: 0.75rem;
  width: 340px;
  backdrop-filter: blur(12px);
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.5);
}
.card h2 {
  font-size: 0.8rem;
  color: #94a3b8;
  margin-bottom: 0.6rem;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  padding-bottom: 0.4rem;
}

.input-group {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 0.75rem;
}

input[type="text"],
input[type="password"] {
  flex: 1;
  padding: 0.5rem 0.6rem;
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.5);
  color: #fff;
  font-size: 0.85rem;
  outline: none;
}
input[type="text"]:focus,
input[type="password"]:focus {
  border-color: #3a7bd5;
}

button {
  padding: 0.5rem 0.9rem;
  border: none;
  border-radius: 6px;
  background: linear-gradient(135deg, #3a7bd5, #00d2ff);
  color: #fff;
  font-size: 0.85rem;
  font-weight: 600;
  cursor: pointer;
}
button:hover {
  opacity: 0.9;
}
button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.success {
  color: #4ade80;
}
.error {
  color: #f87171;
}
.loading {
  color: #facc15;
}
</style>
