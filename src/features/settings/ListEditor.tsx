import { useState, type KeyboardEvent as ReactKeyboardEvent, type ReactNode } from "react";
import { X } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { GlassButton } from "@/components/GlassButton";

type ListEditorProps = {
  id: string;
  value: string[];
  onChange: (next: string[]) => void;
  placeholder: string;
  addLabel: string;
  emptyMessage: string;
  removeLabel: string;
};

export const ListEditor = ({
  id,
  value,
  onChange,
  placeholder,
  addLabel,
  emptyMessage,
  removeLabel,
}: ListEditorProps): ReactNode => {
  const [draft, setDraft] = useState("");

  const appendDraft = (): void => {
    const next = draft.trim();
    if (next.length === 0) return;
    if (value.includes(next)) {
      setDraft("");
      return;
    }
    onChange([...value, next]);
    setDraft("");
  };

  const removeAt = (index: number): void => {
    onChange(value.filter((_, idx) => idx !== index));
  };

  const onInputKey = (event: ReactKeyboardEvent<HTMLInputElement>): void => {
    if (event.key === "Enter") {
      event.preventDefault();
      appendDraft();
    }
  };

  return (
    <div className="list-editor">
      {value.length === 0 ? (
        <p className="list-editor__empty">{emptyMessage}</p>
      ) : (
        <ul className="list-editor__items">
          {value.map((entry, index) => (
            <li key={`${entry}-${index}`} className="list-editor__chip">
              <span className="list-editor__chip-text">{entry}</span>
              <IconButton label={`${removeLabel}: ${entry}`} onClick={() => removeAt(index)}>
                <X size={12} strokeWidth={1.5} />
              </IconButton>
            </li>
          ))}
        </ul>
      )}
      <div className="list-editor__add">
        <input
          type="text"
          className="input list-editor__input"
          id={`${id}-input`}
          value={draft}
          placeholder={placeholder}
          aria-label={placeholder}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={onInputKey}
        />
        <GlassButton variant="primary" onClick={appendDraft} disabled={draft.trim().length === 0}>
          {addLabel}
        </GlassButton>
      </div>
    </div>
  );
};
