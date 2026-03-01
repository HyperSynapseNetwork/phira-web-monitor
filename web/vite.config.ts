import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  server: {
    port: 3000,
    fs: {
      // Allow serving files from the monitor/pkg directory
      allow: [".."],
    },
  },
  build: {
    target: "esnext",
  },
});
