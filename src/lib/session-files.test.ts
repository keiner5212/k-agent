import { describe, expect, it } from "vitest";

import { diffEditorValue, toonFieldValue } from "./session-files";

describe("toonFieldValue", () => {
  it("extracts a quoted multi-line string with \\n escapes", () => {
    const source =
      'path: "smoke/note.txt"\nstartLine: 1\nendLine: 3\ncontent: "1: line A\\n2: line B\\n3: line C"';
    expect(toonFieldValue(source, "content")).toBe("1: line A\n2: line B\n3: line C");
  });

  it("extracts a quoted string with embedded double quotes", () => {
    const source = 'body: "say \\"hi\\" then leave"';
    expect(toonFieldValue(source, "body")).toBe('say "hi" then leave');
  });

  it("extracts an unquoted scalar", () => {
    expect(toonFieldValue("status: ok\npath: foo", "status")).toBe("ok");
  });

  it("returns empty string when the key is absent", () => {
    expect(toonFieldValue("status: ok", "missing")).toBe("");
  });

  it("keeps unchanged lines out of a multi-hunk diff", () => {
    const before = "a\nkeep\nb\nkeep\nc\n";
    const after = "A\nkeep\nB\nkeep\nC\n";
    const packed = diffEditorValue(before, after, 0);
    const removed = packed.lineKinds.filter((kind) => kind === "remove");
    const added = packed.lineKinds.filter((kind) => kind === "add");
    expect(removed).toHaveLength(3);
    expect(added).toHaveLength(3);
    expect(packed.value.split("\n").filter((line) => line === "keep")).toHaveLength(0);
  });

  it("does not match a key that is a prefix of another key", () => {
    const source = "pathLong: a\npath: b";
    expect(toonFieldValue(source, "path")).toBe("b");
  });
});
