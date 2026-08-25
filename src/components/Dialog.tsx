import { X } from "lucide-react";
import { useEffect, useId, useRef, type CSSProperties, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { IconButton } from "./IconButton";

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

type DialogSize = "narrow" | "default" | "wide";
type DialogPlacement = "fill" | "center";

type DialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  titleKey: string;
  children: ReactNode;
  footer?: ReactNode;
  size?: DialogSize;
  placement?: DialogPlacement;
  surfaceStyle?: CSSProperties;
};

export const Dialog = ({
  open,
  onOpenChange,
  titleKey,
  children,
  footer,
  size = "default",
  placement = "fill",
  surfaceStyle,
}: DialogProps): ReactNode => {
  const { t } = useTranslation();
  const titleId = useId();
  const panelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const previous = document.activeElement;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";

    const panel = panelRef.current;
    const focusables = (): HTMLElement[] =>
      panel
        ? [...panel.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
            (node) => node.offsetParent !== null,
          )
        : [];

    focusables()[0]?.focus();

    const onKey = (event: KeyboardEvent): void => {
      if (event.key === "Escape") {
        if (event.defaultPrevented) return;
        event.preventDefault();
        onOpenChange(false);
        return;
      }
      if (event.key !== "Tab" || !panel) return;
      const items = focusables();
      if (items.length === 0) return;
      const first = items[0];
      const last = items[items.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };

    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      document.body.style.overflow = previousOverflow;
      if (previous instanceof HTMLElement) previous.focus();
    };
  }, [open, onOpenChange]);

  if (!open || typeof document === "undefined") return null;

  return createPortal(
    <div className="dialog-root">
      <div className="dialog-overlay" onClick={() => onOpenChange(false)} />
      <div
        className="dialog-surface"
        data-size={size}
        data-placement={placement}
        style={surfaceStyle}
      >
        <div
          ref={panelRef}
          className="dialog-panel"
          role="dialog"
          aria-modal="true"
          aria-labelledby={titleId}
        >
          <div className="dialog-header">
            <h2 id={titleId} className="dialog-title">
              {t(titleKey)}
            </h2>
            <IconButton label={t("settings.close")} onClick={() => onOpenChange(false)}>
              <X size={14} strokeWidth={1.5} />
            </IconButton>
          </div>
          <div className="dialog-body">{children}</div>
          {footer ? <div className="dialog-footer">{footer}</div> : null}
        </div>
      </div>
    </div>,
    document.body,
  );
};
