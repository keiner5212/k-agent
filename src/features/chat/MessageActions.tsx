import { useCallback, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Copy, Maximize2, RotateCcw } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { parseShellMessage } from "@/lib/shell";
import { useShellOutputStore } from "@/lib/shell-output";
import { useRewindConfirmStore } from "@/lib/rewind-confirm";
import { useSessionsStore } from "@/lib/sessions";
import type { ChatMessage } from "@/types/chat";

type MessageActionsProps = {
  message: ChatMessage;
};

const COPY_FEEDBACK_MS = 1200;

const copyText = async (text: string): Promise<boolean> => {
  if (typeof navigator === "undefined" || !navigator.clipboard) return false;
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
};

export const MessageActions = ({ message }: MessageActionsProps): ReactNode => {
  const { t } = useTranslation();
  const requestRewind = useRewindConfirmStore((state) => state.requestRewind);
  const openShellOutput = useShellOutputStore((state) => state.openFor);
  const sending = useSessionsStore((state) => state.sending);
  const sendingSessionId = useSessionsStore((state) => state.sendingSessionId);
  const activeSessionId = useSessionsStore((state) => state.activeSessionId);
  const isSendingHere = sending && sendingSessionId === activeSessionId;
  const [copied, setCopied] = useState(false);

  const hasContent = message.content.trim().length > 0;
  const isUser = message.role === "user";
  const isStreaming = Boolean(message.streaming);
  const canCopy = hasContent && !isStreaming;
  const canRewind = isUser && !isSendingHere;
  const canExpand = message.kind === "shell" && hasContent;

  const handleCopy = useCallback(async () => {
    if (!canCopy) return;
    const ok = await copyText(message.content);
    if (!ok) return;
    setCopied(true);
    window.setTimeout(() => setCopied(false), COPY_FEEDBACK_MS);
  }, [canCopy, message.content]);

  const handleRewind = useCallback(() => {
    if (!canRewind) return;
    requestRewind(message.id);
  }, [canRewind, message.id, requestRewind]);

  const handleExpand = useCallback(() => {
    if (!canExpand || !activeSessionId) return;
    const { command } = parseShellMessage(message.content);
    openShellOutput(message.id, command, activeSessionId);
  }, [canExpand, message.id, message.content, activeSessionId, openShellOutput]);

  return (
    <div className="chat-message__actions" data-role={message.role}>
      {canExpand ? (
        <IconButton
          label={t("chat.shell.expand")}
          className="chat-message__action"
          onClick={handleExpand}
        >
          <Maximize2 size={14} strokeWidth={1.5} />
        </IconButton>
      ) : null}
      {canCopy ? (
        <IconButton
          label={copied ? t("chat.message.copied") : t("chat.message.copy")}
          className="chat-message__action"
          onClick={() => {
            void handleCopy();
          }}
        >
          <Copy size={14} strokeWidth={1.5} />
        </IconButton>
      ) : null}
      {canRewind ? (
        <IconButton
          label={t("chat.message.rewind")}
          className="chat-message__action"
          onClick={handleRewind}
        >
          <RotateCcw size={14} strokeWidth={1.5} />
        </IconButton>
      ) : null}
    </div>
  );
};
