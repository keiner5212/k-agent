import { type CSSProperties, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";
import { LineEditor, type LineKind } from "@/components/LineEditor";

export const READ_ONLY_EDITOR_SURFACE: CSSProperties = {
  height: "calc(100vh - var(--titlebar-height) - var(--space-6))",
  maxHeight: "calc(100vh - var(--titlebar-height) - var(--space-6))",
};

type ReadOnlyEditorDialogProps = {
  open: boolean;
  titleKey: string;
  value: string;
  path?: string;
  startLine?: number;
  lineNumbers?: number[];
  lineKinds?: LineKind[];
  onOpenChange: (open: boolean) => void;
};

export const ReadOnlyEditorDialog = ({
  open,
  titleKey,
  value,
  path,
  startLine,
  lineNumbers,
  lineKinds,
  onOpenChange,
}: ReadOnlyEditorDialogProps): ReactNode => {
  const { t } = useTranslation();
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      titleKey={titleKey}
      size="wide"
      surfaceStyle={READ_ONLY_EDITOR_SURFACE}
    >
      {open ? (
        <div className="skill-editor">
          {path ? (
            <div className="skill-editor__head">
              <div className="skill-editor__path" title={path}>
                {path}
              </div>
            </div>
          ) : null}
          <LineEditor
            value={value}
            onChange={() => undefined}
            readOnly
            startLine={startLine}
            lineNumbers={lineNumbers}
            lineKinds={lineKinds}
          />
          <div className="form-actions">
            <GlassButton variant="secondary" onClick={() => onOpenChange(false)}>
              {t("agents.default.close")}
            </GlassButton>
          </div>
        </div>
      ) : null}
    </Dialog>
  );
};
