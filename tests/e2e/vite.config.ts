import { resolve } from "node:path";

import tailwindcss from "@tailwindcss/vite";
import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  build: {
    rollupOptions: {
      input: {
        basic: resolve(import.meta.dirname, "apps/basic/index.html"),
        "scoped-token": resolve(import.meta.dirname, "apps/scoped-token/index.html"),
        transactions: resolve(import.meta.dirname, "apps/transactions/index.html"),
        errors: resolve(import.meta.dirname, "apps/errors/index.html"),
        "values-golden": resolve(import.meta.dirname, "apps/values-golden/index.html"),
        "listing-golden": resolve(import.meta.dirname, "apps/listing-golden/index.html"),
        "auth-matrix": resolve(import.meta.dirname, "apps/auth-matrix/index.html"),
      },
    },
  },
});
