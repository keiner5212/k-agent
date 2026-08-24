import * as DialogPrimitive from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

type DialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  titleKey: string;
  children: ReactNode;
  footer?: ReactNode;
};

export const Dialog = ({
  open,
  onOpenChange,
  titleKey,
  children,
  footer,
}: DialogProps): ReactNode => {
  const { t } = useTranslation();

  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="dialog-overlay" />
        <DialogPrimitive.Content className="dialog-content" aria-describedby={undefined}>
          <div className="dialog-header">
            <DialogPrimitive.Title className="dialog-title">{t(titleKey)}</DialogPrimitive.Title>
            <DialogPrimitive.Close asChild>
              <button type="button" className="icon-button" aria-label={t("settings.close")}>
                <X size={16} strokeWidth={1.5} />
              </button>
            </DialogPrimitive.Close>
          </div>
          <div className="dialog-body">{children}</div>
          {footer ? <div className="dialog-footer">{footer}</div> : null}
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
};
