import { describe, expect, it } from "vitest";
import { WireReader, WireWriter } from "../src/wire.js";

/**
 * A writer bound to a caller-owned region writes straight into that memory and
 * never reads its local buffer, so it no longer allocates one. These pin the
 * behaviour that change must not alter.
 */
describe("region-bound writer", () => {
  const memory = () => new WebAssembly.Memory({ initial: 1 });

  it("writes into the region it was given", () => {
    const wasm = memory();
    const offset = 128;
    const writer = WireWriter.withWasmRegion(offset, 24, () => wasm.buffer);

    writer.writeF64(1.5);
    writer.writeF64(-2.25);
    writer.writeU64(7n);

    const reader = new WireReader(wasm.buffer, offset, false, 24);
    expect(reader.readF64()).toBe(1.5);
    expect(reader.readF64()).toBe(-2.25);
    expect(reader.readU64()).toBe(7n);
  });

  it("refuses to grow past the region", () => {
    const wasm = memory();
    const writer = WireWriter.withWasmRegion(0, 8, () => wasm.buffer);

    writer.writeF64(1);
    expect(() => writer.writeF64(2)).toThrow();
  });

  it("can be reset and written again", () => {
    const wasm = memory();
    const writer = WireWriter.withWasmRegion(64, 8, () => wasm.buffer);

    writer.writeF64(3.5);
    writer.reset();
    writer.writeF64(9.25);

    expect(new WireReader(wasm.buffer, 64, false, 8).readF64()).toBe(9.25);
  });

  it("shares one allocator across writers without crossing their regions", () => {
    const wasm = memory();
    const allocator = WireWriter.fixedRegionAllocator(() => wasm.buffer);
    const first = WireWriter.withWasmRegion(0, 8, allocator.buffer, allocator);
    const second = WireWriter.withWasmRegion(64, 8, allocator.buffer, allocator);

    first.writeF64(1.5);
    second.writeF64(2.5);

    expect(new WireReader(wasm.buffer, 0, false, 8).readF64()).toBe(1.5);
    expect(new WireReader(wasm.buffer, 64, false, 8).readF64()).toBe(2.5);
  });
});

describe("local writer still grows", () => {
  it("keeps earlier bytes when it reallocates", () => {
    // The grow path reads the local buffer through the lazy accessor now.
    const writer = new WireWriter(8);
    for (let index = 0; index < 64; index++) {
      writer.writeU32(index);
    }

    const bytes = writer.getBytes();
    const reader = new WireReader(bytes.buffer as ArrayBuffer, bytes.byteOffset);
    for (let index = 0; index < 64; index++) {
      expect(reader.readU32()).toBe(index);
    }
  });
});
