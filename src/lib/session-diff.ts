export type DiffLineKind = "context" | "add" | "remove";

type DiffRow = {
  kind: DiffLineKind;
  text: string;
  line: number;
};

export type DiffEditorValue = {
  value: string;
  lineNumbers: number[];
  lineKinds: DiffLineKind[];
};

const splitLines = (source: string): string[] => (source.length === 0 ? [] : source.split("\n"));

const MAX_DIFF_CELLS = 4_000_000;

type LineSpan = {
  oldStart: number;
  oldEnd: number;
  newStart: number;
  newEnd: number;
};

const changedSpans = (oldLines: string[], newLines: string[]): LineSpan[] => {
  const cells = oldLines.length * newLines.length;
  if (cells > MAX_DIFF_CELLS) {
    return [prefixSuffixSpan(oldLines, newLines)];
  }
  const width = oldLines.length + 1;
  const table = new Uint32Array(width * (newLines.length + 1));
  for (let newIndex = 1; newIndex <= newLines.length; newIndex += 1) {
    for (let oldIndex = 1; oldIndex <= oldLines.length; oldIndex += 1) {
      const at = newIndex * width + oldIndex;
      if (oldLines[oldIndex - 1] === newLines[newIndex - 1]) {
        table[at] = (table[(newIndex - 1) * width + (oldIndex - 1)] ?? 0) + 1;
      } else {
        const up = table[(newIndex - 1) * width + oldIndex] ?? 0;
        const left = table[newIndex * width + (oldIndex - 1)] ?? 0;
        table[at] = up > left ? up : left;
      }
    }
  }
  const matches: Array<{ oldIndex: number; newIndex: number }> = [];
  let oldIndex = oldLines.length;
  let newIndex = newLines.length;
  while (oldIndex > 0 && newIndex > 0) {
    const at = newIndex * width + oldIndex;
    if (
      oldLines[oldIndex - 1] === newLines[newIndex - 1] &&
      table[at] === (table[(newIndex - 1) * width + (oldIndex - 1)] ?? 0) + 1
    ) {
      matches.push({ oldIndex: oldIndex - 1, newIndex: newIndex - 1 });
      oldIndex -= 1;
      newIndex -= 1;
    } else if ((table[(newIndex - 1) * width + oldIndex] ?? 0) >= (table[at - 1] ?? 0)) {
      newIndex -= 1;
    } else {
      oldIndex -= 1;
    }
  }
  matches.reverse();
  const spans: LineSpan[] = [];
  let oldCursor = 0;
  let newCursor = 0;
  const pushGap = (oldEnd: number, newEnd: number) => {
    if (oldCursor === oldEnd && newCursor === newEnd) {
      return;
    }
    spans.push({ oldStart: oldCursor, oldEnd, newStart: newCursor, newEnd });
  };
  for (const match of matches) {
    pushGap(match.oldIndex, match.newIndex);
    oldCursor = match.oldIndex + 1;
    newCursor = match.newIndex + 1;
  }
  pushGap(oldLines.length, newLines.length);
  return spans;
};

const prefixSuffixSpan = (oldLines: string[], newLines: string[]): LineSpan => {
  let start = 0;
  while (
    start < oldLines.length &&
    start < newLines.length &&
    oldLines[start] === newLines[start]
  ) {
    start += 1;
  }
  let oldEnd = oldLines.length;
  let newEnd = newLines.length;
  while (oldEnd > start && newEnd > start && oldLines[oldEnd - 1] === newLines[newEnd - 1]) {
    oldEnd -= 1;
    newEnd -= 1;
  }
  return { oldStart: start, oldEnd, newStart: start, newEnd };
};

const buildDiffRows = (before: string, after: string, context: number): DiffRow[] => {
  const oldLines = splitLines(before);
  const newLines = splitLines(after);
  const spans = changedSpans(oldLines, newLines);
  const rows: DiffRow[] = [];
  let oldCursor = 0;
  let newCursor = 0;
  for (const span of spans) {
    const oldFrom = Math.max(oldCursor, span.oldStart - context);
    const skipped = oldFrom - oldCursor;
    oldCursor = oldFrom;
    newCursor += skipped;
    while (oldCursor < span.oldStart) {
      rows.push({ kind: "context", text: oldLines[oldCursor] ?? "", line: newCursor + 1 });
      oldCursor += 1;
      newCursor += 1;
    }
    while (oldCursor < span.oldEnd) {
      rows.push({ kind: "remove", text: oldLines[oldCursor] ?? "", line: oldCursor + 1 });
      oldCursor += 1;
    }
    while (newCursor < span.newEnd) {
      rows.push({ kind: "add", text: newLines[newCursor] ?? "", line: newCursor + 1 });
      newCursor += 1;
    }
    const oldContextEnd = Math.min(oldLines.length, span.oldEnd + context);
    while (oldCursor < oldContextEnd) {
      rows.push({ kind: "context", text: oldLines[oldCursor] ?? "", line: newCursor + 1 });
      oldCursor += 1;
      newCursor += 1;
    }
  }
  return rows;
};

export const diffEditorValue = (before: string, after: string, context = 3): DiffEditorValue => {
  if (before === after) {
    return { value: after, lineNumbers: [], lineKinds: [] };
  }
  const rows = buildDiffRows(before, after, context);
  return {
    value: rows.map((row) => row.text).join("\n"),
    lineNumbers: rows.map((row) => row.line),
    lineKinds: rows.map((row) => row.kind),
  };
};
