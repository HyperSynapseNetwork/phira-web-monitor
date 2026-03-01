<template>
  <n-config-provider :theme="darkTheme" :theme-overrides="themeOverrides">
    <n-global-style />
    <div class="app">
      <nav class="top-nav">
        <h1 class="brand">Phira Web Monitor</h1>
        <n-space :size="4">
          <n-button
            v-for="tab in tabs"
            :key="tab.key"
            :quaternary="activeTab !== tab.key"
            :type="activeTab === tab.key ? 'primary' : 'default'"
            size="small"
            @click="activeTab = tab.key"
          >
            {{ tab.label }}
          </n-button>
        </n-space>
      </nav>
      <!-- Both views stay mounted to preserve WebGL/WS state.
           visibility:hidden (not display:none) properly hides GPU-composited WebGL canvases. -->
      <div class="views-container">
        <PlayerView
          :class="['view', { 'view-hidden': activeTab !== 'play' }]"
        />
        <MonitorView
          :class="['view', { 'view-hidden': activeTab !== 'monitor' }]"
        />
      </div>
    </div>
  </n-config-provider>
</template>

<script setup lang="ts">
import { ref } from "vue";
import {
  darkTheme,
  NConfigProvider,
  NGlobalStyle,
  NButton,
  NSpace,
} from "naive-ui";
import type { GlobalThemeOverrides } from "naive-ui";
import PlayerView from "./views/PlayerView.vue";
import MonitorView from "./views/MonitorView.vue";

const tabs = [
  { key: "play" as const, label: "Play" },
  { key: "monitor" as const, label: "Monitor" },
];
const activeTab = ref<"play" | "monitor">("monitor");

const themeOverrides: GlobalThemeOverrides = {
  common: {
    bodyColor: "#0d1117",
    cardColor: "rgba(13, 17, 23, 0.85)",
    primaryColor: "#3a7bd5",
    primaryColorHover: "#4a8be5",
    primaryColorPressed: "#2a6bc5",
    primaryColorSuppl: "#00d2ff",
    successColor: "#4ade80",
    successColorHover: "#6ee7a0",
    successColorPressed: "#3acd70",
    warningColor: "#facc15",
    warningColorHover: "#fbd635",
    warningColorPressed: "#e0b800",
    errorColor: "#f87171",
    errorColorHover: "#fa9090",
    errorColorPressed: "#e65050",
    textColor1: "#ffffff",
    textColor2: "#c9d1d9",
    textColor3: "#8b949e",
    borderColor: "rgba(255, 255, 255, 0.1)",
    borderRadius: "6px",
    fontFamily:
      '"Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu, Cantarell, "Open Sans", "Helvetica Neue", sans-serif',
    fontSize: "14px",
  },
  Card: {
    borderRadius: "12px",
    paddingMedium: "1.25rem",
    borderColor: "rgba(255, 255, 255, 0.1)",
    color: "rgba(13, 17, 23, 0.85)",
  },
  Input: {
    color: "rgba(255, 255, 255, 0.06)",
    colorFocus: "rgba(255, 255, 255, 0.09)",
    border: "1px solid rgba(255, 255, 255, 0.15)",
    borderHover: "1px solid rgba(58, 123, 213, 0.6)",
    borderFocus: "1px solid #3a7bd5",
    boxShadowFocus: "0 0 0 2px rgba(58, 123, 213, 0.2)",
    borderRadius: "6px",
  },
  Button: {
    borderRadiusMedium: "6px",
    borderRadiusSmall: "6px",
    fontWeightStrong: "600",
    textColorPrimary: "#fff",
    textColorHoverPrimary: "#fff",
    textColorPressedPrimary: "#fff",
    textColorFocusPrimary: "#fff",
    textColorSuccess: "#fff",
    textColorHoverSuccess: "#fff",
    textColorPressedSuccess: "#fff",
    textColorFocusSuccess: "#fff",
    textColorWarning: "#fff",
    textColorHoverWarning: "#fff",
    textColorPressedWarning: "#fff",
    textColorFocusWarning: "#fff",
    textColorError: "#fff",
    textColorHoverError: "#fff",
    textColorPressedError: "#fff",
    textColorFocusError: "#fff",
    textColorDisabledPrimary: "rgba(255, 255, 255, 0.5)",
    textColorDisabledSuccess: "rgba(255, 255, 255, 0.5)",
    textColorDisabledWarning: "rgba(255, 255, 255, 0.5)",
    textColorDisabledError: "rgba(255, 255, 255, 0.5)",
  },
  Divider: {
    color: "rgba(255, 255, 255, 0.06)",
  },
  Select: {
    peers: {
      InternalSelection: {
        color: "#0b1120",
        borderHover: "rgba(58, 123, 213, 0.5)",
        borderFocus: "rgba(58, 123, 213, 0.5)",
        boxShadowFocus: "0 0 0 2px rgba(58, 123, 213, 0.15)",
        borderActive: "rgba(58, 123, 213, 0.5)",
        boxShadowActive: "0 0 0 2px rgba(58, 123, 213, 0.15)",
      },
      InternalSelectMenu: {
        color: "#0b1120",
      },
    },
  },
};
</script>

<style>
/* ── Minimal Global Reset ─────────────────────────────────────────── */
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
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

/* Both views overlap; visibility:hidden hides GPU-composited WebGL canvases
   (display:none from v-show does NOT reliably hide them). */
.views-container {
  flex: 1;
  position: relative;
  min-height: 0;
  overflow: hidden;
}
.view {
  position: absolute;
  inset: 0;
  z-index: 1;
}
.view-hidden {
  visibility: hidden;
  pointer-events: none;
  z-index: 0;
}

/* ── Top Navigation Bar ───────────────────────────────────────────── */
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
  background-clip: text;
  -webkit-text-fill-color: transparent;
}
</style>
