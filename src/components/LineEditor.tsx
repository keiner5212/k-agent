import { useLayoutEffect, useMemo, useRef, useState, type ReactNode, type UIEvent } from "react";
import { useUndoRedoKeydown } from "@/lib/use-undo-redo-keydown";
import { useUndoableText } from "@/lib/undoable-text";
import { useSettingsStore } from "@/lib/settings";

export type LineKind = "context" | "add" | "remove";

type LineEditorProps = {
  value: string;
  onChange: (next: string) => void;
  readOnly?: boolean;
  maxLines?: number;
  id?: string;
  startLine?: number;
  lineNumbers?: number[];
  lineKinds?: LineKind[];
};

const originLine = (startLine: number | undefined): number =>
  startLine && startLine > 0 ? Math.floor(startLine) : 1;

const markForKind = (kind: LineKind): string => {
  if (kind === "add") return "+";
  if (kind === "remove") return "-";
  return "";
};

export const LineEditor = ({
  value,
  onChange,
  readOnly,
  maxLines,
  id,
  startLine,
  lineNumbers,
  lineKinds,
}: LineEditorProps): ReactNode => {
  const gutterInnerRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const measureRef = useRef<HTMLDivElement>(null);
  const keybindings = useSettingsStore((state) => state.keybindings);
  const { pushChange, undo, redo } = useUndoableText(value, onChange);
  const [lineHeights, setLineHeights] = useState<number[]>([0]);
  const [measureTick, setMeasureTick] = useState(0);
  const rowMode = Boolean(lineKinds && lineKinds.length > 0);
  const origin = originLine(startLine);

  useUndoRedoKeydown(textareaRef, keybindings, undo, redo, !readOnly && !rowMode);

  const logicalLines = useMemo(() => {
    if (value.length === 0) return [""];
    return value.split("\n");
  }, [value]);

  const syncGutter = (top: number): void => {
    const inner = gutterInnerRef.current;
    if (!inner) return;
    inner.style.transform = `translateY(${-top}px)`;
  };

  const onScroll = (event: UIEvent<HTMLTextAreaElement>): void => {
    syncGutter(event.currentTarget.scrollTop);
  };

  useLayoutEffect(() => {
    if (rowMode) return;
    syncGutter(textareaRef.current?.scrollTop ?? 0);
  }, [rowMode, value, logicalLines.length, lineHeights]);

  useLayoutEffect(() => {
    if (rowMode) return;
    const textarea = textareaRef.current;
    const measure = measureRef.current;
    if (!textarea || !measure) return;

    const styles = window.getComputedStyle(textarea);
    const paddingLeft = parseFloat(styles.paddingLeft);
    const paddingRight = parseFloat(styles.paddingRight);
    const contentWidth = Math.max(1, textarea.clientWidth - paddingLeft - paddingRight);
    measure.style.width = `${contentWidth}px`;
    measure.style.fontFamily = styles.fontFamily;
    measure.style.fontSize = styles.fontSize;
    measure.style.lineHeight = styles.lineHeight;
    measure.style.whiteSpace = "pre-wrap";
    measure.style.wordBreak = "break-word";

    const heights = logicalLines.map((line) => {
      const row = document.createElement("div");
      row.style.whiteSpace = "pre-wrap";
      row.style.wordBreak = "break-word";
      row.style.lineHeight = styles.lineHeight;
      row.textContent = line.length === 0 ? " " : line;
      measure.appendChild(row);
      const height = row.offsetHeight;
      measure.removeChild(row);
      return height;
    });

    setLineHeights(heights.length > 0 ? heights : [0]);
  }, [rowMode, logicalLines, value, measureTick]);

  useLayoutEffect(() => {
    if (rowMode) return;
    const textarea = textareaRef.current;
    if (!textarea) return;
    const observer = new ResizeObserver(() => {
      setMeasureTick((tick) => tick + 1);
    });
    observer.observe(textarea);
    return () => observer.disconnect();
  }, [rowMode]);

  if (rowMode && lineKinds) {
    return (
      <div className="line-editor line-editor--rows">
        {logicalLines.map((text, index) => {
          const kind = lineKinds[index] ?? "context";
          const number = lineNumbers?.[index] ?? origin + index;
          return (
            <div key={index} className="line-editor__row" data-kind={kind}>
              <span className="line-editor__gutter-line">{number}</span>
              <span className="line-editor__mark" aria-hidden="true">
                {markForKind(kind)}
              </span>
              <span className="line-editor__code">{text.length === 0 ? " " : text}</span>
            </div>
          );
        })}
      </div>
    );
  }

  return (
    <div className="line-editor">
      <div className="line-editor__gutter" aria-hidden="true">
        <div ref={gutterInnerRef} className="line-editor__gutter-inner">
          {logicalLines.map((_, index) => (
            <div
              key={index}
              className="line-editor__gutter-line"
              style={{ minHeight: lineHeights[index] ?? undefined }}
            >
              {lineNumbers?.[index] ?? origin + index}
            </div>
          ))}
        </div>
      </div>
      <textarea
        ref={textareaRef}
        id={id}
        className="line-editor__textarea"
        value={value}
        onChange={(event) => {
          let next = event.target.value;
          if (maxLines !== undefined) {
            const lines = next.split("\n");
            if (lines.length > maxLines) next = lines.slice(0, maxLines).join("\n");
          }
          pushChange(next);
        }}
        onScroll={onScroll}
        spellCheck={false}
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        readOnly={readOnly}
        wrap="soft"
      />
      <div ref={measureRef} className="line-editor__measure" aria-hidden="true" />
    </div>
  );
};
