import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig(() => {
  const performanceCapture = (
    globalThis as typeof globalThis & {
      process?: { env?: Record<string, string | undefined> };
    }
  ).process?.env?.OSTUDY_UI_PERFORMANCE_CAPTURE === "1";
  return {
    plugins: [react(), tailwindcss()],
    clearScreen: false,
    define: {
      __OSTUDY_UI_PERFORMANCE_CAPTURE__: JSON.stringify(performanceCapture),
    },
    resolve: {
      alias: {
        "@": fileURLToPath(new URL("./src", import.meta.url)),
        ...(performanceCapture ? { "react-dom/client": "react-dom/profiling" } : {}),
      },
    },
    server: {
      host: "127.0.0.1",
      port: 1420,
      strictPort: true,
    },
    envPrefix: ["VITE_", "TAURI_"],
  };
});
