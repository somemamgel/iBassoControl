// Wire encoding and validation live exclusively in ibasso-protocol (Rust/WASM).
import { LocalizedError } from "./i18n";
import * as wasm from "@ibasso/wasm";
type Metadata = {
    deviceFilter: HIDDeviceFilter;
    product: string;
    volumeTable: number[];
    filters: Record<"d1" | "d2" | "d3" | "d4" | "d5", number>;
    gains: Record<"low" | "medium" | "high", number>;
    outputs: Record<"normal" | "turbo", number>;
};
const metadata: Metadata = JSON.parse(wasm.metadata_json());
export const DEVICE_FILTER = metadata.deviceFilter;
export const PRODUCT = metadata.product;
export const VOLUME_TABLE = metadata.volumeTable;
export const FILTERS = metadata.filters;
export const GAINS = metadata.gains;
export const OUTPUTS = metadata.outputs;
export type Setting =
    | { kind: "volume"; level: number; balance: number }
    | { kind: "filter"; value: keyof typeof FILTERS }
    | { kind: "gain"; value: keyof typeof GAINS }
    | { kind: "output"; value: keyof typeof OUTPUTS }
    | { kind: "eco"; value: boolean };
export interface WriteStep {
    write: Uint8Array;
    readback: Uint8Array;
    expected: number;
}
type QueryName =
    | "firmware"
    | "volumeLeft"
    | "volumeRight"
    | "filterLeft"
    | "filterRight"
    | "gainLeft"
    | "gainRight"
    | "outputLeft"
    | "outputRight"
    | "eco";
export const queries = Object.fromEntries(
    (JSON.parse(wasm.queries_json()) as [QueryName, number[]][]).map(([key, bytes]) => [key, Uint8Array.from(bytes)]),
) as Record<QueryName, Uint8Array>;
export type Status = Record<QueryName, number>;
// wasm-bindgen throws Rust string errors; normalize them for browser error handlers.
function call<T>(run: () => T): T {
    try {
        return run();
    } catch (error) {
        throw new LocalizedError("protocolError", { detail: error instanceof Error ? error.message : String(error) });
    }
}
export function plan(setting: Setting): WriteStep[] {
    const steps: { write: number[]; readback: number[]; expected: number }[] = JSON.parse(
        call(() => wasm.plan_json(JSON.stringify(setting))),
    );
    return steps.map((step) => ({
        ...step,
        write: Uint8Array.from(step.write),
        readback: Uint8Array.from(step.readback),
    }));
}
export const readPreamble = (report: Uint8Array): Uint8Array | null => call(() => wasm.preamble(report)) ?? null;
export const responseData = (bytes: Uint8Array, id: number): Uint8Array | null =>
    call(() => wasm.decode_response(bytes, id)) ?? null;
export const readValue = (report: Uint8Array, data: Uint8Array): number => call(() => wasm.read_value(report, data));
export const volumeLevel = (raw: number): number | null =>
    Number.isInteger(raw) && raw >= 0 && raw <= 255 ? (wasm.volume_level(raw) ?? null) : null;
export const hex = (value: number): string => "0x" + value.toString(16).padStart(2, "0");
