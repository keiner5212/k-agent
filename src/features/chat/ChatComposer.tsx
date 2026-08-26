import { useCallback, useRef, type KeyboardEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ArrowUp, Paperclip, Play } from "lucide-react";
import { GlassButton } from "@/components/GlassButton";
import { IconButton } from "@/components/IconButton";
import { useChatStore } from "@/lib/chat";
import { useComposerStore } from "@/lib/composer";
import { matchActionSlashCommand } from "@/lib/slash-commands";
import { useSelectionStore } from "@/lib/selected-model";
import { useSettingsStore } from "@/lib/settings";
import { useSessionsStore } from "@/lib/sessions";
import { useUndoRedoKeydown } from "@/lib/use-undo-redo-keydown";
import { useUndoableText } from "@/lib/undoable-text";
import { AgentSelector } from "./AgentSelector";
import { ComposerShellChip } from "./ComposerModeChip";
import { ComposerTextarea } from "./ComposerTextarea";
import { ContextUsage } from "./ContextUsage";
import { EffortSelector } from "./EffortSelector";
import { ModelSelector } from "./ModelSelector";
import { useComposerFocusKeys } from "./use-composer-focus";

export const ChatComposer = (): ReactNode => {
  const { t } = useTranslation();
  const value = useComposerStore((state) => state.value);
  const mode = useComposerStore((state) => state.mode);
  const setValue = useComposerStore((state) => state.setValue);
  const clearComposer = useComposerStore((state) => state.clear);
  const selection = useSelectionStore((state) => state.selection);
  const sending = useChatStore((state) => state.sending);
  const hydrated = useChatStore((state) => state.hydrated);
  const sendMessage = useChatStore((state) => state.send);
  const rewindLastUserMessage = useSessionsStore((state) => state.rewindLastUserMessage);
  const keybindings = useSettingsStore((state) => state.keybindings);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { pushChange, undo, redo } = useUndoableText(value, setValue);
  const shellMode = mode === "shell";
  const canSend = Boolean(selection) && hydrated && value.trim().length > 0 && !sending;

  const runSlashAction = useCallback(
    (commandId: string): void => {
      if (commandId === "undo") {
        rewindLastUserMessage();
      }
    },
    [rewindLastUserMessage],
  );

  const handleSend = useCallback(async () => {
    if (!canSend) return;
    const text = value;
    const action = matchActionSlashCommand(text);
    if (action) {
      clearComposer();
      runSlashAction(action.id);
      textareaRef.current?.focus();
      return;
    }
    clearComposer();
    await sendMessage(text);
    textareaRef.current?.focus();
  }, [canSend, clearComposer, runSlashAction, sendMessage, value]);

  const handleComposerKeyDown = useCallback(
    (event: KeyboardEvent<HTMLTextAreaElement>) => {
      if (shellMode) return;
      if (event.key !== "Enter" || event.shiftKey || event.nativeEvent.isComposing) return;
      if (!canSend) return;
      event.preventDefault();
      void handleSend();
    },
    [canSend, handleSend, shellMode],
  );

  useUndoRedoKeydown(textareaRef, keybindings, undo, redo);
  useComposerFocusKeys(textareaRef, pushChange);

  return (
    <footer className="chat-composer" {...(shellMode ? { "data-mode": "shell" } : {})}>
      <div className="chat-composer__toolbar">
        {shellMode ? <ComposerShellChip /> : null}
        {!shellMode ? <AgentSelector /> : null}
        {!shellMode ? <ModelSelector /> : null}
        {!shellMode ? <EffortSelector /> : null}
        <ContextUsage />
      </div>
      <div className="chat-composer__field">
        <ComposerTextarea
          value={value}
          mode={mode}
          onChange={pushChange}
          textareaRef={textareaRef}
          onKeyDown={handleComposerKeyDown}
        />
        <div className="chat-composer__actions">
          {!shellMode ? (
            <IconButton
              label={t("chat.composer.attach")}
              className="chat-composer__attach"
              onClick={() => undefined}
            >
              <Paperclip size={16} strokeWidth={1.5} />
            </IconButton>
          ) : null}
          <GlassButton
            variant="primary"
            className={`chat-composer__send${shellMode ? " chat-composer__send--shell" : ""}`}
            aria-label={
              shellMode
                ? canSend
                  ? t("chat.composer.run")
                  : t("chat.composer.disabledHintShell")
                : canSend
                  ? t("chat.composer.send")
                  : t("chat.composer.disabledHint")
            }
            title={
              shellMode
                ? canSend
                  ? t("chat.composer.run")
                  : t("chat.composer.disabledHintShell")
                : canSend
                  ? t("chat.composer.send")
                  : t("chat.composer.disabledHint")
            }
            onClick={() => {
              void handleSend();
            }}
            disabled={!canSend}
          >
            {shellMode ? <Play size={16} strokeWidth={2} /> : <ArrowUp strokeWidth={2} />}
          </GlassButton>
        </div>
      </div>
    </footer>
  );
};
