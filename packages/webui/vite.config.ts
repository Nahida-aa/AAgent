import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import { paraglideVitePlugin } from "@inlang/paraglide-js";
import tailwindcss from "@tailwindcss/vite";
import { devtools } from "@tanstack/devtools-vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [
    devtools(),
    paraglideVitePlugin({
      project: "../../packages/shared/i18n/project.inlang",
      outdir: "../../packages/shared/i18n/paraglide",
      strategy: ["cookie", "preferredLanguage", "baseLocale"],
    }),
    tailwindcss(),
    tanstackRouter({ target: "solid", autoCodeSplitting: true }),
    solid(),
  ],

  clearScreen: false,
  server: {
    port: 1430,
    proxy: {
      "/api": {
        target: "http://localhost:3000",
        changeOrigin: true,
        ws: true,
        rewrite: (path: string) => path.replace(/^\/api/, ""),
      },
    },
    strictPort: true,
  },
}));
