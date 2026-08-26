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
import { segmentComposerHighlights } from "@/lib/composer-highlights";
import { applyComposerAgentCycle, matchesAgentCycle } from "@/lib/composer-agents";
import { tryMentionBackspace } from "@/lib/composer-mention-edits";
import type { ComposerMode } from "@/lib/composer";
import { useSettingsStore } from "@/lib/settings";
import { FileMentionMenu } from "./FileMentionMenu";
import { SlashCommandMenu } from "./SlashCommandMenu";
import { useFileMentions } from "./use-file-mentions";
import { useSlashCommands } from "./use-slash-commands";

const EMPTY_SLASH_NAMES = new Set<string>();

type ComposerTextareaProps = {
  value: string;
  mode: ComposerMode;
  onChange: (next: string) => void;
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  onKeyDown?: (event: React.KeyboardEvent<HTMLTextAreaElement>) => void;
  onPaste?: (event: React.ClipboardEvent<HTMLTextAreaElement>) => void;
};

export const ComposerTextarea = ({
  value,
  mode,
  onChange,
  textareaRef,
  onKeyDown,
  onPaste,
}: ComposerTextareaProps): ReactNode => {
  const { t } = useTranslation();
  const backdropRef = useRef<HTMLDivElement>(null);
  const [cursor, setCursor] = useState(0);

  const shellMode = mode === "shell";
  const slashEnabled = !shellMode;
  const agentCycleBinding = useSettingsStore((state) => state.keybindings["chat.agentCycle"]);

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
    menuOpen: atMenuOpen,
    items: atItems,
    tooMany: atTooMany,
    activeIndex: atActiveIndex,
    mentionPaths,
    loading: atLoading,
    error: atError,
    handleKeyDown: handleAtKeyDown,
    pickItem: pickAtItem,
    activeMention,
  } = useFileMentions({
    value,
    cursor,
    onApply: applyValue,
  });

  const {
    menuOpen: slashMenuOpen,
    items: slashItems,
    tooMany: slashTooMany,
    activeIndex: slashActiveIndex,
    query: slashQuery,
    slashNames,
    loading: slashLoading,
    error: slashError,
    handleKeyDown: handleSlashKeyDown,
    pickItem: pickSlashItem,
  } = useSlashCommands({
    value,
    cursor,
    enabled: slashEnabled,
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

  const slashHighlightNames = slashEnabled ? slashNames : EMPTY_SLASH_NAMES;

  const segments = useMemo(
    () => segmentComposerHighlights(value, mentionPaths, slashHighlightNames),
    [mentionPaths, slashHighlightNames, value],
  );

  const handleChange = (event: ChangeEvent<HTMLTextAreaElement>): void => {
    onChange(event.target.value);
    const next = event.target.selectionStart ?? 0;
    setCursor((current) => (current === next ? current : next));
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>): void => {
    if (matchesAgentCycle(event.nativeEvent, agentCycleBinding)) {
      if (!shellMode && !slashMenuOpen && !atMenuOpen) {
        applyComposerAgentCycle(1);
      }
      event.preventDefault();
      return;
    }
    if (event.key === "Backspace" && !event.shiftKey) {
      const field = textareaRef.current;
      const selStart = field?.selectionStart ?? cursor;
      const selEnd = field?.selectionEnd ?? cursor;
      if (selStart === selEnd) {
        const edited = tryMentionBackspace(value, selStart);
        if (edited) {
          event.preventDefault();
          applyValue(edited.next, edited.cursor);
          return;
        }
      }
    }
    if (slashEnabled && slashMenuOpen && handleSlashKeyDown(event)) return;
    if (atMenuOpen && handleAtKeyDown(event)) return;
    onKeyDown?.(event);
  };

  const placeholder = shellMode
    ? t("chat.composer.placeholderShell")
    : t("chat.composer.placeholder");

  return (
    <div className="chat-composer__input-stack" {...(shellMode ? { "data-mode": "shell" } : {})}>
      {shellMode ? (
        <span className="chat-composer__shell-prompt" aria-hidden="true">
          $
        </span>
      ) : null}
      <div ref={backdropRef} className="chat-composer__input-backdrop" aria-hidden="true">
        {!value ? (
          <span className="chat-composer__input-placeholder">{placeholder}</span>
        ) : segments.length === 1 && !segments[0]?.mention ? (
          segments[0]?.text
        ) : (
          segments.map((segment, index) =>
            segment.mention ? (
              <mark key={index} className="chat-composer__mention">
                {segment.text}
              </mark>
            ) : (
              <span key={index}>{segment.text}</span>
            ),
          )
        )}
      </div>
      <textarea
        ref={textareaRef}
        className="chat-composer__input chat-composer__input--mentions"
        value={value}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        onPaste={onPaste}
        onClick={syncCursor}
        onSelect={syncCursor}
        onScroll={syncScroll}
        rows={2}
        aria-label={placeholder}
        aria-expanded={atMenuOpen || (slashEnabled && slashMenuOpen)}
        spellCheck={false}
      />
      {slashEnabled ? (
        <SlashCommandMenu
          open={slashMenuOpen}
          items={slashItems}
          query={slashQuery}
          activeIndex={slashActiveIndex}
          loading={slashLoading}
          error={slashError}
          tooMany={slashTooMany}
          onPick={pickSlashItem}
        />
      ) : null}
      <FileMentionMenu
        open={atMenuOpen}
        items={atItems}
        query={activeMention?.query ?? ""}
        activeIndex={atActiveIndex}
        loading={atLoading}
        error={atError}
        tooMany={atTooMany}
        onPick={pickAtItem}
      />
    </div>
  );
};
