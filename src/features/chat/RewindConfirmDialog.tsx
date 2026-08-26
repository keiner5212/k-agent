import { useMemo, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";
import { useRewindConfirmStore } from "@/lib/rewind-confirm";
import { useSessionsStore } from "@/lib/sessions";

export const RewindConfirmDialog = (): ReactNode => {
  const { t } = useTranslation();
  const open = useRewindConfirmStore((state) => state.open);
  const targetMessageId = useRewindConfirmStore((state) => state.targetMessageId);
  const cancel = useRewindConfirmStore((state) => state.cancel);
  const rewindTo = useSessionsStore((state) => state.rewindTo);

  const removalCount = useMemo(() => {
    if (!open || !targetMessageId) return 0;
    const sessionId = useSessionsStore.getState().activeSessionId;
    if (!sessionId) return 0;
    const session = useSessionsStore.getState().sessions.find((item) => item.id === sessionId);
    if (!session) return 0;
    const index = session.messages.findIndex((m) => m.id === targetMessageId);
    if (index < 0) return 0;
    return session.messages.length - index;
  }, [targetMessageId, open]);

  const handleConfirm = (): void => {
    if (!targetMessageId) {
      cancel();
      return;
    }
    rewindTo(targetMessageId);
    cancel();
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) cancel();
      }}
      titleKey="chat.rewind.title"
      placement="center"
      size="narrow"
    >
      <div className="delete-skill">
        <p className="delete-skill__body">{t("chat.rewind.body", { count: removalCount })}</p>
        <div className="form-actions">
          <GlassButton variant="secondary" onClick={cancel}>
            {t("chat.rewind.cancel")}
          </GlassButton>
          <GlassButton variant="danger" onClick={handleConfirm}>
            {t("chat.rewind.confirm")}
          </GlassButton>
        </div>
      </div>
    </Dialog>
  );
};
