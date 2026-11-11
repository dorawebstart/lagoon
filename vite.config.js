import { defineConfig } from "vite";

// Налаштування Vite під Tauri: https://v2.tauri.app/start/frontend/vite/
export default defineConfig({
  // Не очищати термінал, щоб було видно помилки Rust
  clearScreen: false,
  server: {
    // Tauri очікує фіксований порт (див. devUrl у src-tauri/tauri.conf.json)
    port: 1420,
    strictPort: true,
    watch: {
      // Не перезапускати Vite, коли змінюються Rust-файли
      ignored: ["**/src-tauri/**"],
    },
  },
  // Змінні середовища з префіксом TAURI_ теж доступні у фронтенді
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    // WebView2 на Windows базується на Chromium, тож можна сучасний JS
    target: "chrome105",
    outDir: "dist",
  },
});
