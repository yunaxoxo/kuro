import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: {
    // The sample programs live in ../examples and are imported with `?raw`.
    fs: { allow: [".."] },
  },
  worker: { format: "es" },
});
