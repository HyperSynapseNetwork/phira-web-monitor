---
sources:
  - "web/package.json"
  - "web/vite.config.ts"
  - "web/src/**/*"
  - "web/.env"
  - "web/public/assets/**/*"
---

# Module: web

## Responsibility

`web` is the Vue 3 shell that provides the user-facing browser UI for both single-chart playback and authenticated multiplayer monitoring.

## Entry Surface

- `src/main.ts` mounts the app and registers i18n.
- `src/App.vue` owns top-level theme, locale switching, and view tab selection.
- `src/views/PlayerView.vue` drives `ChartPlayer`.
- `src/views/MonitorView.vue` handles auth, WebSocket connection state, room join/leave actions, and scene/canvas coordination for `GameMonitor`.

## Key Dependencies

- `vue`, `naive-ui`, and `vue-i18n` for UI composition and localization.
- Local `monitor-client` package from `web/pkg` for WASM bindings.
- Vite for dev server and bundling.

## Notable Patterns

- `VITE_API_BASE` is the only checked-in frontend environment variable and defaults to `http://localhost:3080` in `web/.env`.
- The app keeps both major views mounted to preserve WebGL and WebSocket state when switching tabs.
- Resource-pack media is loaded from `web/public/assets/respack/default/` and then handed to the WASM runtime.
