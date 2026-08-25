import { useLayoutEffect, useMemo, useRef, type ReactNode, type UIEvent } from "react";

type LineEditorProps = {
  value: string;
  onChange: (next: string) => void;
  readOnly?: boolean;
  maxLines?: number;
  id?: string;
};

export const LineEditor = ({
  value,
  onChange,
  readOnly,
  maxLines,
  id,
}: LineEditorProps): ReactNode => {
  const gutterInnerRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const lineCount = useMemo(() => {
    if (value.length === 0) return 1;
    return value.split("\n").length;
  }, [value]);

  const lineNumbers = useMemo(
    () => Array.from({ length: lineCount }, (_, index) => index + 1),
    [lineCount],
  );

  const syncGutter = (top: number): void => {
    const inner = gutterInnerRef.current;
    if (!inner) return;
    inner.style.transform = `translateY(${-top}px)`;
  };

  const onScroll = (event: UIEvent<HTMLTextAreaElement>): void => {
    syncGutter(event.currentTarget.scrollTop);
  };

  useLayoutEffect(() => {
    syncGutter(textareaRef.current?.scrollTop ?? 0);
  }, [value, lineCount]);

  return (
    <div className="line-editor">
      <div className="line-editor__gutter" aria-hidden="true">
        <div ref={gutterInnerRef} className="line-editor__gutter-inner">
          {lineNumbers.map((num) => (
            <div key={num} className="line-editor__gutter-line">
              {num}
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
          onChange(next);
        }}
        onScroll={onScroll}
        spellCheck={false}
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        readOnly={readOnly}
        wrap="off"
      />
    </div>
  );
};
