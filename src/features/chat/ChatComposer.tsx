import { useRef, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ArrowUp, Paperclip, Play } from "lucide-react";
import { GlassButton } from "@/components/GlassButton";
import { IconButton } from "@/components/IconButton";
import { useComposerStore } from "@/lib/composer";
import { useSelectionStore } from "@/lib/selected-model";
import { useSettingsStore } from "@/lib/settings";
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
  const selection = useSelectionStore((state) => state.selection);
  const keybindings = useSettingsStore((state) => state.keybindings);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { pushChange, undo, redo } = useUndoableText(value, setValue);
  const shellMode = mode === "shell";
  const canSend = Boolean(selection) && value.trim().length > 0;

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
            onClick={() => undefined}
            disabled={!canSend}
          >
            {shellMode ? <Play size={16} strokeWidth={2} /> : <ArrowUp strokeWidth={2} />}
          </GlassButton>
        </div>
      </div>
    </footer>
  );
};
