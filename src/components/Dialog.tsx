import * as DialogPrimitive from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { GlassSurface } from "./GlassSurface";
import { IconButton } from "./IconButton";

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
        <DialogPrimitive.Content
          className="dialog-anchor"
          aria-describedby={undefined}
          onCloseAutoFocus={(event) => event.preventDefault()}
        >
          <GlassSurface className="dialog-surface" cornerRadius={4} elasticity={0}>
            <div className="dialog-header">
              <DialogPrimitive.Title className="dialog-title">{t(titleKey)}</DialogPrimitive.Title>
              <DialogPrimitive.Close asChild>
                <IconButton label={t("settings.close")}>
                  <X size={14} strokeWidth={1.5} />
                </IconButton>
              </DialogPrimitive.Close>
            </div>
            <div className="dialog-body">{children}</div>
            {footer ? <div className="dialog-footer">{footer}</div> : null}
          </GlassSurface>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
};
