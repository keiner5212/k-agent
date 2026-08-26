import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { FileDiff } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import type { LineKind } from "@/components/LineEditor";
import { ReadOnlyEditorDialog } from "@/features/chat/ReadOnlyEditorDialog";
import {
  collectSessionChanges,
  diffEditorValue,
  readSessionFileRevision,
  type SessionChangedFile,
} from "@/lib/session-files";
import { selectActiveMessages, useSessionsStore } from "@/lib/sessions";

export const ChangeBar = (): ReactNode => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [diff, setDiff] = useState<{
    path: string;
    value: string;
    lineNumbers?: number[];
    lineKinds?: LineKind[];
  } | null>(null);
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
      const packed = diffEditorValue(before.content, after.content);
      setDiff({
        path: file.path,
        value: packed.value,
        lineNumbers: packed.lineNumbers,
        lineKinds: packed.lineKinds,
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
        open={diff !== null}
        titleKey="chat.tools.editTitle"
        value={diff?.value ?? ""}
        path={diff?.path}
        lineNumbers={diff?.lineNumbers}
        lineKinds={diff?.lineKinds}
        onOpenChange={(open) => {
          if (!open) setDiff(null);
        }}
      />
    </>
  );
};
