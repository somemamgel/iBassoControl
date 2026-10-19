import { defineConfig } from "vite";
import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
    plugins: [tailwindcss()],
    build: { target: "es2022" },
    resolve: { alias: { "@ibasso/wasm": fileURLToPath(new URL("./src/wasm.ts", import.meta.url)) } },
    server: { port: 5173, strictPort: true },
    preview: { port: 4173, strictPort: true },
});
