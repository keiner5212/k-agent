import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { FileDiff } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import {
  ReadOnlyEditorDialog,
  READ_ONLY_EDITOR_SURFACE,
} from "@/features/chat/ReadOnlyEditorDialog";
import {
  collectSessionChanges,
  readSessionFileRevision,
  unifiedDiff,
  type SessionChangedFile,
} from "@/lib/session-files";
import { selectActiveMessages, useSessionsStore } from "@/lib/sessions";

export const ChangeBar = (): ReactNode => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [diff, setDiff] = useState<{ path: string; value: string } | null>(null);
  const sessionId = useSessionsStore((state) => state.activeSessionId);
  const messages = useSessionsStore(selectActiveMessages);
  const files = collectSessionChanges(messages);
  const added = files.reduce((sum, file) => sum + file.added, 0);
  const removed = files.reduce((sum, file) => sum + file.removed, 0);
  const lines = added + removed;

  const openFile = async (file: SessionChangedFile): Promise<void> => {
    if (!sessionId) return;
    try {
      const [before, after] = await Promise.all([
        readSessionFileRevision(sessionId, file.callId, "before"),
        readSessionFileRevision(sessionId, file.callId, "after"),
      ]);
      setDiff({
        path: file.path,
        value: unifiedDiff(file.path, before.content, after.content),
      });
    } catch {
      setDiff({ path: file.path, value: "" });
    }
  };

  return (
    <>
      <div className="change-bar">
        <button
          type="button"
          className="change-bar__stat"
          onClick={() => setOpen(true)}
          aria-label={t("chat.changes.label")}
        >
          <FileDiff size={12} strokeWidth={1.5} aria-hidden="true" />
          <span className="change-bar__copy">
            <span className="change-bar__value">{files.length}</span>
            <span>{t("chat.changes.files", { count: files.length })}</span>
          </span>
        </button>
        <span className="change-bar__stat">
          <span className="change-bar__copy">
            <span className="change-bar__value">{lines}</span>
            <span>{t("chat.changes.lines", { count: lines })}</span>
            <span className="change-bar__added">+{added}</span>
            <span className="change-bar__removed">-{removed}</span>
          </span>
        </span>
      </div>
      <Dialog
        open={open}
        onOpenChange={setOpen}
        titleKey="chat.changes.title"
        size="narrow"
        placement="center"
        surfaceStyle={{ width: "min(420px, calc(100vw - var(--space-6)))" }}
      >
        {files.length === 0 ? (
          <p className="change-dialog__empty">{t("chat.changes.empty")}</p>
        ) : (
          <ul className="change-dialog__list">
            {files.map((file) => (
              <li key={file.path} className="change-dialog__row">
                <button
                  type="button"
                  className="change-dialog__open"
                  title={file.path}
                  onClick={() => {
                    void openFile(file);
                  }}
                >
                  <span className="change-dialog__path">{file.path}</span>
                  <span className="change-bar__added">+{file.added}</span>
                  <span className="change-bar__removed">-{file.removed}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </Dialog>
      <ReadOnlyEditorDialog
        open={diff !== null && !diff.value}
        titleKey="chat.tools.editTitle"
        value=""
        path={diff?.path}
        onOpenChange={(open) => {
          if (!open) setDiff(null);
        }}
      />
      <Dialog
        open={diff !== null && Boolean(diff.value)}
        onOpenChange={(next) => {
          if (!next) setDiff(null);
        }}
        titleKey="chat.tools.editTitle"
        size="wide"
        surfaceStyle={READ_ONLY_EDITOR_SURFACE}
      >
        {diff?.value ? <pre className="chat-diff">{diff.value}</pre> : null}
      </Dialog>
    </>
  );
};
