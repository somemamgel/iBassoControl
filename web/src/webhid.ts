import { LocalizedError } from "./i18n";
import {
    DEVICE_FILTER,
    PRODUCT,
    plan,
    queries,
    readPreamble,
    readValue,
    responseData,
    hex,
    type Setting,
    type Status,
} from "./protocol";

export interface HidPort {
    open(): Promise<void>;
    close(): Promise<void>;
    send(data: Uint8Array): Promise<void>;
    subscribe(listener: (reportId: number, data: Uint8Array) => void): () => void;
}
export type Trace = (direction: "TX" | "RX", bytes: Uint8Array) => void;

export function supportsDevice(device: HIDDevice): boolean {
    return (
        device.vendorId === DEVICE_FILTER.vendorId &&
        device.productId === DEVICE_FILTER.productId &&
        device.productName.trim().toLowerCase() === PRODUCT.toLowerCase()
    );
}
export function browserPort(device: HIDDevice): HidPort {
    if (!supportsDevice(device)) throw new LocalizedError("wrongDevice");
    const collections: HIDCollectionInfo[] = [];
    const walk = (items: HIDCollectionInfo[]) => {
        for (const item of items) {
            collections.push(item);
            walk(item.children ?? []);
        }
    };
    walk(device.collections);
    if (
        !collections.some((c) => c.inputReports?.some((r) => r.reportId === 0)) ||
        !collections.some((c) => c.outputReports?.some((r) => r.reportId === 0))
    ) {
        throw new LocalizedError("reportsMissing");
    }
    return {
        open: () => device.open(),
        close: () => device.close(),
        // WebHID takes the ID separately: payload is 16 bytes, NOT hidapi's 17 bytes.
        send: (data) => device.sendReport(0, Uint8Array.from(data)),
        subscribe: (listener) => {
            const handler = (event: HIDInputReportEvent) =>
                listener(
                    event.reportId,
                    new Uint8Array(event.data.buffer, event.data.byteOffset, event.data.byteLength),
                );
            device.addEventListener("inputreport", handler);
            return () => device.removeEventListener("inputreport", handler);
        },
    };
}

interface Pending {
    id: number;
    receive(data: Uint8Array): void;
    fail(error: Error): void;
}

/** One operation at a time. A timed-out/failed session cannot consume late replies. */
export class WebHidClient {
    private pending: Pending | null = null;
    private unsubscribe: (() => void) | null = null;
    private healthy = false;
    private busy = false;
    private opened = false;
    private disposed = false;
    constructor(
        private port: HidPort,
        private timeoutMs = 1000,
        private trace?: Trace,
    ) {
        if (!Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 30000)
            throw new LocalizedError("timeoutRange");
    }
    async open(): Promise<void> {
        if (this.opened || this.unsubscribe || this.disposed) throw new LocalizedError("sessionUsed");
        this.unsubscribe = this.port.subscribe((id, bytes) => {
            if (id !== 0) return;
            this.trace?.("RX", bytes);
            if (!this.pending) return; // Drop startup/idle events; never cache them for future commands.
            try {
                const data = responseData(bytes, this.pending.id);
                if (data !== null) this.pending.receive(data);
            } catch (error) {
                this.pending?.fail(asError(error));
            }
        });
        try {
            await this.port.open();
            if (this.disposed) {
                await this.port.close();
                throw new LocalizedError("cancelled");
            }
            this.healthy = true;
            this.opened = true;
        } catch (error) {
            this.unsubscribe?.();
            this.unsubscribe = null;
            throw error;
        }
    }
    async close(): Promise<void> {
        this.disposed = true;
        this.healthy = false;
        this.pending?.fail(new LocalizedError("closed"));
        this.unsubscribe?.();
        this.unsubscribe = null;
        if (this.opened) {
            this.opened = false;
            await this.port.close();
        }
    }
    private async exclusive<T>(work: () => Promise<T>): Promise<T> {
        if (!this.healthy) throw new LocalizedError("unhealthy");
        if (this.busy) throw new LocalizedError("operationBusy");
        this.busy = true;
        try {
            return await work();
        } catch (error) {
            this.healthy = false;
            throw error;
        } finally {
            this.busy = false;
        }
    }
    private exchange(report: Uint8Array): Promise<Uint8Array> {
        if (!this.healthy) return Promise.reject(new LocalizedError("invalidSession"));
        return new Promise((resolve, reject) => {
            let sent = false;
            let received: Uint8Array | null = null;
            let settled = false;
            const cleanup = () => {
                clearTimeout(timer);
                if (this.pending === pending) this.pending = null;
            };
            const fail = (error: Error) => {
                if (settled) return;
                settled = true;
                this.healthy = false;
                cleanup();
                reject(error);
            };
            const finish = () => {
                if (settled || !sent || received === null) return;
                settled = true;
                cleanup();
                resolve(received);
            };
            const pending: Pending = {
                id: report[0],
                fail,
                receive: (data) => {
                    received = data;
                    finish();
                },
            };
            // Timer covers sendReport AND response. Register handler before sending: ACK may arrive first.
            const timer = setTimeout(
                () => fail(new LocalizedError("timeout", { command: hex(report[0]) })),
                this.timeoutMs,
            );
            this.pending = pending;
            this.trace?.("TX", report);
            try {
                this.port.send(report).then(
                    () => {
                        sent = true;
                        finish();
                    },
                    (error) => fail(asError(error)),
                );
            } catch (error) {
                fail(asError(error));
            }
        });
    }
    private async readByte(report: Uint8Array): Promise<number> {
        const preamble = readPreamble(report);
        if (preamble) await this.exchange(preamble);
        const data = await this.exchange(report);
        return readValue(report, data);
    }
    status(): Promise<Status> {
        return this.exclusive(async () => {
            const result = {} as Status;
            for (const key of Object.keys(queries) as (keyof Status)[]) result[key] = await this.readByte(queries[key]);
            return result;
        });
    }
    apply(setting: Setting): Promise<void> {
        const steps = plan(setting); // Validate all input before device I/O.
        return this.exclusive(async () => {
            for (const [index, step] of steps.entries()) {
                try {
                    await this.exchange(step.write);
                    const value = await this.readByte(step.readback);
                    if (value !== step.expected)
                        throw new LocalizedError("mismatch", { expected: hex(step.expected), actual: hex(value) });
                } catch (error) {
                    throw new LocalizedError("partial", {
                        step: index + 1,
                        total: steps.length,
                        detail: asError(error),
                    });
                }
            }
        });
    }
}
export function asError(error: unknown): Error {
    return error instanceof Error ? error : new Error(String(error));
}

/** Held for the entire session to prevent same-origin tabs racing MAP/read transactions. */
export function acquireDeviceLease(): Promise<() => void> {
    if (!navigator.locks) return Promise.reject(new LocalizedError("noLocks"));
    return new Promise((resolve, reject) => {
        navigator.locks
            .request("ibasso-dc04-pro", { ifAvailable: true }, async (lock) => {
                if (!lock) {
                    reject(new LocalizedError("otherTab"));
                    return;
                }
                await new Promise<void>((release) => resolve(release));
            })
            .catch(reject);
    });
}
