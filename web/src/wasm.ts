import init from "../pkg/ibasso_wasm.js";
import url from "../pkg/ibasso_wasm_bg.wasm?url";
await init({ module_or_path: url });
export * from "../pkg/ibasso_wasm.js";
