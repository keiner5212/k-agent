import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { FileDiff } from "lucide-react";
import { Dialog } from "@/components/Dialog";

const MOCK_CHANGED_FILES = [
  { path: "src/features/chat/ChatComposer.tsx", added: 48, removed: 12 },
  { path: "src/styles/chat.css", added: 62, removed: 18 },
  { path: "src/features/sessions/SessionsSidebar.tsx", added: 11, removed: 4 },
  { path: "src/i18n/locales/en.json", added: 7, removed: 2 },
] as const;

const MOCK_FILES = MOCK_CHANGED_FILES.length;
const MOCK_ADDED = MOCK_CHANGED_FILES.reduce((sum, file) => sum + file.added, 0);
const MOCK_REMOVED = MOCK_CHANGED_FILES.reduce((sum, file) => sum + file.removed, 0);

export const ChangeBar = (): ReactNode => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const lines = MOCK_ADDED + MOCK_REMOVED;

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
            <span className="change-bar__value">{MOCK_FILES}</span>
            <span>{t("chat.changes.files", { count: MOCK_FILES })}</span>
          </span>
        </button>
        <span className="change-bar__stat">
          <span className="change-bar__copy">
            <span className="change-bar__value">{lines}</span>
            <span>{t("chat.changes.lines", { count: lines })}</span>
            <span className="change-bar__added">+{MOCK_ADDED}</span>
            <span className="change-bar__removed">-{MOCK_REMOVED}</span>
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
        <ul className="change-dialog__list">
          {MOCK_CHANGED_FILES.map((file) => (
            <li key={file.path} className="change-dialog__row">
              <span className="change-dialog__path" title={file.path}>
                {file.path}
              </span>
              <span className="change-bar__added">+{file.added}</span>
              <span className="change-bar__removed">-{file.removed}</span>
            </li>
          ))}
        </ul>
      </Dialog>
    </>
  );
};
