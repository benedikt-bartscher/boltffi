import { describe, expect, it } from "vitest";
import { WireReader, WireWriter } from "../src/wire.js";

function readerOver(writer: WireWriter): WireReader {
  const bytes = writer.getBytes();
  return new WireReader(bytes.buffer as ArrayBuffer, bytes.byteOffset);
}

/**
 * Both decoders size the result array up front instead of growing it. That is
 * only equivalent if every index is written: an array built with
 * `new Array(len)` and left partly unassigned has holes, which `Object.keys`,
 * `in` and `forEach` all skip, so a value could go missing without the length
 * or a `toEqual` comparison noticing.
 */
function expectNoHoles(values: readonly unknown[]): void {
  expect(Object.keys(values)).toHaveLength(values.length);
  for (let index = 0; index < values.length; index++) {
    expect(index in values).toBe(true);
  }
}

describe("readArray", () => {
  it("decodes an empty array", () => {
    const writer = new WireWriter();
    writer.writeArray<number>([], (value) => writer.writeU32(value));

    const values = readerOver(writer).readArray(() => 0);
    expect(values).toEqual([]);
    expectNoHoles(values);
  });

  it("preserves order and leaves no holes", () => {
    const source = Array.from({ length: 300 }, (_, index) => index * 3);
    const writer = new WireWriter();
    writer.writeArray(source, (value) => writer.writeU32(value));

    const reader = readerOver(writer);
    const values = reader.readArray(() => reader.readU32());

    expect(values).toEqual(source);
    expectNoHoles(values);
  });

  it("keeps nested arrays independent", () => {
    const writer = new WireWriter();
    writer.writeArray(
      [
        [1, 2],
        [3],
        [],
      ],
      (inner) => writer.writeArray(inner, (value) => writer.writeI32(value))
    );

    const reader = readerOver(writer);
    const values = reader.readArray(() => reader.readArray(() => reader.readI32()));

    expect(values).toEqual([[1, 2], [3], []]);
    values.forEach(expectNoHoles);
    expectNoHoles(values);
  });
});

describe("readBoolArray", () => {
  it("decodes an empty array", () => {
    const writer = new WireWriter();
    writer.writeArray<boolean>([], (value) => writer.writeBool(value));

    const values = readerOver(writer).readBoolArray();
    expect(values).toEqual([]);
    expectNoHoles(values);
  });

  it("widens every byte and leaves no holes", () => {
    const source = Array.from({ length: 257 }, (_, index) => index % 3 === 0);
    const writer = new WireWriter();
    writer.writeArray(source, (value) => writer.writeBool(value));

    const values = readerOver(writer).readBoolArray();

    expect(values).toEqual(source);
    expectNoHoles(values);
  });

  it("treats any non-zero byte as true", () => {
    // The wire only ever carries 0 or 1, but the decoder should not assume it.
    const writer = new WireWriter();
    writer.writeU32(3);
    writer.writeU8(0);
    writer.writeU8(1);
    writer.writeU8(0xff);

    expect(readerOver(writer).readBoolArray()).toEqual([false, true, true]);
  });
});
