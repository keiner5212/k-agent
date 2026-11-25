import { X } from "lucide-react";
import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type CSSProperties,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { IconButton } from "./IconButton";
import {
  bindDialog,
  markDialogClosing,
  releaseDialog,
  subscribeDialogs,
  topDialogId,
} from "@/lib/dialog-stack";

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

type DialogSize = "narrow" | "default" | "wide";
type DialogPlacement = "fill" | "center";
type DialogPhase = "open" | "closed";

const EXIT_MS = 240;

const windowMotionEnabled = (): boolean => {
  if (typeof document === "undefined") return false;
  const root = document.documentElement;
  if (root.dataset.animations === "disabled") return false;
  if (root.dataset.windowAnimations === "disabled") return false;
  return !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
};

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
  const stackId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const onOpenChangeRef = useRef(onOpenChange);
  const [present, setPresent] = useState(open);
  const [phase, setPhase] = useState<DialogPhase>(open ? "open" : "closed");
  const topId = useSyncExternalStore(subscribeDialogs, topDialogId, topDialogId);
  const isTop = topId === null || topId === stackId;

  useLayoutEffect(() => {
    onOpenChangeRef.current = onOpenChange;
  }, [onOpenChange]);

  useEffect(() => {
    if (open) {
      let inner = 0;
      const outer = requestAnimationFrame(() => {
        setPresent(true);
        if (!windowMotionEnabled()) {
          setPhase("open");
          return;
        }
        setPhase("closed");
        inner = requestAnimationFrame(() => setPhase("open"));
      });
      return () => {
        cancelAnimationFrame(outer);
        cancelAnimationFrame(inner);
      };
    }
    markDialogClosing(stackId);
    const closeFrame = window.setTimeout(() => setPhase("closed"), 0);
    const unmount = window.setTimeout(() => setPresent(false), windowMotionEnabled() ? EXIT_MS : 0);
    return () => {
      window.clearTimeout(closeFrame);
      window.clearTimeout(unmount);
    };
  }, [open, stackId]);

  useLayoutEffect(() => {
    if (!present) return;
    bindDialog(stackId, () => onOpenChangeRef.current(false));
    return () => releaseDialog(stackId);
  }, [present, stackId]);

  useEffect(() => {
    if (!open || !present || !isTop) return;
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
  }, [open, present, isTop]);

  if (!present || typeof document === "undefined") return null;

  const onScrimMouseDown = (event: ReactMouseEvent<HTMLDivElement>): void => {
    if (!isTop) return;
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    if (target.closest(".dialog-surface")) return;
    onOpenChange(false);
  };

  return createPortal(
    <div
      className="dialog-root"
      data-state={phase}
      data-top={isTop ? "true" : "false"}
      inert={!isTop ? true : undefined}
      onMouseDown={onScrimMouseDown}
    >
      <div className="dialog-overlay" />
      <div className="dialog-slot" data-size={size} data-placement={placement} style={surfaceStyle}>
        <div className="dialog-surface">
          <div
            ref={panelRef}
            className="dialog-panel"
            role="dialog"
            aria-modal={isTop}
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
      </div>
    </div>,
    document.body,
  );
};
