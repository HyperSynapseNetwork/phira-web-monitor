import { defineConfig } from "vite";

export default defineConfig({
  server: {
    port: 3000,
    fs: {
      // Allow serving files from the monitor/pkg directory
      allow: [".."],
    },
    proxy: {
      // Proxy API requests to our proxy server
      "/api": {
        target: "http://localhost:3080",
        changeOrigin: true,
      },
      "/chart": {
        target: "http://localhost:3080",
        changeOrigin: true,
      },
    },
  },
  build: {
    target: "esnext",
  },
});
