import { createRouter, createWebHistory } from "vue-router";
import PlayView from "./views/PlayView.vue";
import MonitorView from "./views/MonitorView.vue";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", redirect: "/play" },
    { path: "/play", name: "play", component: PlayView },
    { path: "/monitor", name: "monitor", component: MonitorView },
  ],
});

export default router;
