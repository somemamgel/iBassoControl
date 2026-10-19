import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
const root = fileURLToPath(new URL("../../", import.meta.url));
const exe = process.platform === "win32" ? ".exe" : "";
const localCargo = join(root, ".tools", "cargo", "bin", "cargo" + exe);
const cargo = process.env.WASM_CARGO || (existsSync(localCargo) ? localCargo : "cargo");
const localBindgen = join(root, ".tools", "bin", "wasm-bindgen" + exe);
const bindgen = process.env.WASM_BINDGEN || (existsSync(localBindgen) ? localBindgen : "wasm-bindgen");
const env = { ...process.env };
if (cargo === localCargo) {
    env.RUSTUP_HOME = join(root, ".tools", "rustup");
    env.CARGO_HOME = join(root, ".tools", "cargo");
    env.RUSTC = join(root, ".tools", "cargo", "bin", "rustc" + exe);
}
function run(command, args) {
    const result = spawnSync(command, args, { cwd: root, env, stdio: "inherit" });
    if (result.error) throw new Error(command + ": " + result.error.message + ". See README WASM setup.");
    if (result.status !== 0) process.exit(result.status ?? 1);
}
const version = readFileSync(join(root, "Cargo.lock"), "utf8").match(
    /name = "wasm-bindgen"\r?\nversion = "([^"]+)"/,
)?.[1];
const actual = spawnSync(bindgen, ["--version"], { encoding: "utf8", env });
if (!version || actual.status !== 0 || actual.stdout.trim() !== "wasm-bindgen " + version) {
    throw new Error(
        "Install matching CLI: cargo install wasm-bindgen-cli --version " +
            version +
            " --locked (or set WASM_BINDGEN).",
    );
}
run(cargo, [
    "build",
    "--locked",
    "-p",
    "ibasso-wasm",
    "--target",
    "wasm32-unknown-unknown",
    "--release",
    "--target-dir",
    join(root, "target"),
]);
run(bindgen, [
    "--target",
    "web",
    "--out-dir",
    join(root, "web", "pkg"),
    join(root, "target", "wasm32-unknown-unknown", "release", "ibasso_wasm.wasm"),
]);
