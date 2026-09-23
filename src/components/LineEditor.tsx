import { useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useUndoRedoKeydown } from "@/lib/use-undo-redo-keydown";
import { useUndoableText } from "@/lib/undoable-text";
import { useSettingsStore } from "@/lib/settings";
import { highlightLine, highlightLines, resolveLanguage } from "@/lib/syntax-highlight";

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
  language?: string;
  path?: string;
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
  language,
  path,
}: LineEditorProps): ReactNode => {
  const gutterInnerRef = useRef<HTMLDivElement>(null);
  const highlightRef = useRef<HTMLPreElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const measureRef = useRef<HTMLDivElement>(null);
  const keybindings = useSettingsStore((state) => state.keybindings);
  const { pushChange, undo, redo } = useUndoableText(value, onChange);
  const [lineHeights, setLineHeights] = useState<number[]>([0]);
  const [measureTick, setMeasureTick] = useState(0);
  const rowMode = Boolean(lineKinds && lineKinds.length > 0);
  const origin = originLine(startLine);
  const resolvedLanguage = resolveLanguage(language, path, value);

  useUndoRedoKeydown(textareaRef, keybindings, undo, redo, !readOnly && !rowMode);

  const logicalLines = useMemo(() => {
    if (value.length === 0) return [""];
    return value.split("\n");
  }, [value]);

  const highlightedLines = useMemo(() => {
    if (!resolvedLanguage) return null;
    return highlightLines(value, resolvedLanguage);
  }, [value, resolvedLanguage]);

  const syncOverlay = (): void => {
    const textarea = textareaRef.current;
    const top = textarea?.scrollTop ?? 0;
    const gutter = gutterInnerRef.current;
    if (gutter) gutter.style.transform = `translateY(${-top}px)`;
    const highlight = highlightRef.current;
    if (!textarea || !highlight) return;
    const styles = window.getComputedStyle(textarea);
    highlight.style.width = `${textarea.clientWidth}px`;
    highlight.style.paddingTop = styles.paddingTop;
    highlight.style.paddingRight = styles.paddingRight;
    highlight.style.paddingBottom = styles.paddingBottom;
    highlight.style.paddingLeft = styles.paddingLeft;
    highlight.style.fontFamily = styles.fontFamily;
    highlight.style.fontSize = styles.fontSize;
    highlight.style.lineHeight = styles.lineHeight;
    highlight.style.letterSpacing = styles.letterSpacing;
    highlight.style.transform = `translateY(${-top}px)`;
  };

  const onScroll = (): void => {
    syncOverlay();
  };

  useLayoutEffect(() => {
    if (rowMode) return;
    syncOverlay();
  }, [rowMode, value, logicalLines.length, lineHeights, highlightedLines]);

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
      syncOverlay();
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
          const lineHtml = highlightedLines?.[index] ?? highlightLine(text, null);
          return (
            <div key={index} className="line-editor__row" data-kind={kind}>
              <span className="line-editor__gutter-line">{number}</span>
              <span className="line-editor__mark" aria-hidden="true">
                {markForKind(kind)}
              </span>
              <span
                className="line-editor__code"
                data-language={resolvedLanguage ?? undefined}
                dangerouslySetInnerHTML={{ __html: lineHtml }}
              />
            </div>
          );
        })}
      </div>
    );
  }

  return (
    <div className="line-editor" data-language={resolvedLanguage ?? undefined}>
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
        data-overlay={highlightedLines ? "true" : undefined}
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
      {highlightedLines ? (
        <div className="line-editor__highlight" aria-hidden="true">
          <pre ref={highlightRef} className="line-editor__highlight-inner">
            <code
              className={`hljs language-${resolvedLanguage}`}
              dangerouslySetInnerHTML={{
                __html: highlightedLines.join("\n"),
              }}
            />
          </pre>
        </div>
      ) : null}
      <div ref={measureRef} className="line-editor__measure" aria-hidden="true" />
    </div>
  );
};
