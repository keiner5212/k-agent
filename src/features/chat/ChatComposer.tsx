import { type ChangeEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ArrowUp, Paperclip } from "lucide-react";
import { GlassButton } from "@/components/GlassButton";
import { IconButton } from "@/components/IconButton";
import { useComposerStore } from "@/lib/composer";
import { useSelectionStore } from "@/lib/selected-model";
import { AgentSelector } from "./AgentSelector";
import { ContextUsage } from "./ContextUsage";
import { EffortSelector } from "./EffortSelector";
import { ModelSelector } from "./ModelSelector";

export const ChatComposer = (): ReactNode => {
  const { t } = useTranslation();
  const value = useComposerStore((state) => state.value);
  const setValue = useComposerStore((state) => state.setValue);
  const selection = useSelectionStore((state) => state.selection);
  const canSend = Boolean(selection) && value.trim().length > 0;

  const handleChange = (event: ChangeEvent<HTMLTextAreaElement>): void => {
    setValue(event.target.value);
  };

  return (
    <footer className="chat-composer">
      <div className="chat-composer__toolbar">
        <AgentSelector />
        <ModelSelector />
        <EffortSelector />
        <ContextUsage />
      </div>
      <div className="chat-composer__field">
        <textarea
          className="chat-composer__input"
          placeholder={t("chat.composer.placeholder")}
          value={value}
          onChange={handleChange}
          rows={3}
          aria-label={t("chat.composer.placeholder")}
        />
        <div className="chat-composer__actions">
          <IconButton
            label={t("chat.composer.attach")}
            className="chat-composer__attach"
            onClick={() => undefined}
          >
            <Paperclip size={16} strokeWidth={1.5} />
          </IconButton>
          <GlassButton
            variant="primary"
            className="chat-composer__send"
            aria-label={canSend ? t("chat.composer.send") : t("chat.composer.disabledHint")}
            title={canSend ? t("chat.composer.send") : t("chat.composer.disabledHint")}
            onClick={() => undefined}
            disabled={!canSend}
          >
            <ArrowUp strokeWidth={2} />
          </GlassButton>
        </div>
      </div>
    </footer>
  );
};
