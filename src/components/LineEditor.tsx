import { useMemo, type ReactNode } from "react";

type LineEditorProps = {
  value: string;
  onChange: (next: string) => void;
  readOnly?: boolean;
};

export const LineEditor = ({
  value,
  onChange,
  readOnly,
}: LineEditorProps): ReactNode => {
  const lineCount = useMemo(() => {
    if (value.length === 0) return 1;
    return value.split("\n").length;
  }, [value]);

  const lineNumbers = useMemo(
    () => Array.from({ length: lineCount }, (_, index) => index + 1),
    [lineCount],
  );

  return (
    <div className="line-editor">
      <div className="line-editor__gutter" aria-hidden="true">
        {lineNumbers.map((num) => (
          <div key={num} className="line-editor__gutter-line">
            {num}
          </div>
        ))}
      </div>
      <textarea
        className="line-editor__textarea"
        value={value}
        onChange={(event) => onChange(event.target.value)}
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