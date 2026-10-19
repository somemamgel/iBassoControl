import { defineConfig } from "vitest/config";
import { fileURLToPath } from "node:url";
export default defineConfig({
    resolve: { alias: { "@ibasso/wasm": fileURLToPath(new URL("./tests/init-wasm.ts", import.meta.url)) } },
});
