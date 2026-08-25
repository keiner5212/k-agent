import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ChangeEvent,
  type ReactNode,
  type RefObject,
} from "react";
import { useTranslation } from "react-i18next";
import { segmentMentionHighlights } from "@/lib/file-mentions";
import { FileMentionMenu } from "./FileMentionMenu";
import { useFileMentions } from "./use-file-mentions";

type ComposerTextareaProps = {
  value: string;
  onChange: (next: string) => void;
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  onKeyDown?: (event: React.KeyboardEvent<HTMLTextAreaElement>) => void;
};

export const ComposerTextarea = ({
  value,
  onChange,
  textareaRef,
  onKeyDown,
}: ComposerTextareaProps): ReactNode => {
  const { t } = useTranslation();
  const backdropRef = useRef<HTMLDivElement>(null);
  const [cursor, setCursor] = useState(0);

  const applyValue = useCallback(
    (next: string, nextCursor: number) => {
      onChange(next);
      requestAnimationFrame(() => {
        const field = textareaRef.current;
        if (!field) return;
        field.focus();
        field.setSelectionRange(nextCursor, nextCursor);
        setCursor(nextCursor);
      });
    },
    [onChange, textareaRef],
  );

  const {
    menuOpen,
    items,
    tooMany,
    activeIndex,
    mentionPaths,
    loading,
    error,
    handleKeyDown: handleMentionKeyDown,
    pickItem,
    activeMention,
  } = useFileMentions({
    value,
    cursor,
    onApply: applyValue,
  });

  const syncCursor = useCallback(() => {
    const field = textareaRef.current;
    if (!field) return;
    const next = field.selectionStart ?? 0;
    setCursor((current) => (current === next ? current : next));
  }, [textareaRef]);

  const syncScroll = useCallback(() => {
    const field = textareaRef.current;
    const backdrop = backdropRef.current;
    if (!field || !backdrop) return;
    backdrop.scrollTop = field.scrollTop;
    backdrop.scrollLeft = field.scrollLeft;
  }, [textareaRef]);

  useEffect(() => {
    syncScroll();
  }, [value, syncScroll]);

  const segments = useMemo(
    () => segmentMentionHighlights(value, mentionPaths),
    [mentionPaths, value],
  );

  const handleChange = (event: ChangeEvent<HTMLTextAreaElement>): void => {
    onChange(event.target.value);
    const next = event.target.selectionStart ?? 0;
    setCursor((current) => (current === next ? current : next));
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>): void => {
    if (handleMentionKeyDown(event)) return;
    onKeyDown?.(event);
  };

  return (
    <div className="chat-composer__input-stack">
      <div ref={backdropRef} className="chat-composer__input-backdrop" aria-hidden="true">
        {segments.length === 1 && !segments[0]?.mention
          ? segments[0]?.text
          : segments.map((segment, index) =>
              segment.mention ? (
                <mark key={index} className="chat-composer__mention">
                  {segment.text}
                </mark>
              ) : (
                <span key={index}>{segment.text}</span>
              ),
            )}
      </div>
      <textarea
        ref={textareaRef}
        className="chat-composer__input chat-composer__input--mentions"
        placeholder={t("chat.composer.placeholder")}
        value={value}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        onClick={syncCursor}
        onSelect={syncCursor}
        onScroll={syncScroll}
        rows={3}
        aria-label={t("chat.composer.placeholder")}
        aria-expanded={menuOpen}
        spellCheck={false}
      />
      <FileMentionMenu
        open={menuOpen}
        items={items}
        query={activeMention?.query ?? ""}
        activeIndex={activeIndex}
        loading={loading}
        error={error}
        tooMany={tooMany}
        onPick={pickItem}
      />
    </div>
  );
};
