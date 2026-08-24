import { type ChangeEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { SendHorizontal } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { useComposerStore } from "@/lib/composer";
import { useSelectionStore } from "@/lib/selected-model";

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
      <textarea
        className="chat-composer__input"
        placeholder={t("chat.composer.placeholder")}
        value={value}
        onChange={handleChange}
        rows={3}
        aria-label={t("chat.composer.placeholder")}
      />
      <IconButton
        label={canSend ? t("chat.composer.send") : t("chat.composer.disabledHint")}
        className="chat-composer__send"
        onClick={() => undefined}
        disabled={!canSend}
      >
        <SendHorizontal size={16} strokeWidth={1.5} />
      </IconButton>
    </footer>
  );
};
