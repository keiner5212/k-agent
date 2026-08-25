import { Check, ChevronDown } from "lucide-react";
import {
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";

type SelectOption = {
  value: string;
  label: ReactNode;
};

type SelectPlacement = "auto" | "up" | "down";

type SelectProps = {
  value: string;
  onChange: (next: string) => void;
  options: readonly SelectOption[];
  placeholder?: ReactNode;
  ariaLabel?: string;
  id?: string;
  disabled?: boolean;
  placement?: SelectPlacement;
  menuMinWidth?: number;
};

type MenuPos = {
  top?: number;
  bottom?: number;
  left: number;
  width: number;
  maxHeight: number;
  placement: "up" | "down";
};

const MENU_MAX_HEIGHT = 240;
const MENU_MIN_HEIGHT = 48;
const MENU_GAP = 4;
const VIEW_PAD = 8;

const menuPosFromRect = (
  rect: DOMRect,
  placement: SelectPlacement,
  menuMinWidth: number,
): MenuPos => {
  const width = Math.max(rect.width, menuMinWidth);
  const left = Math.min(rect.left, Math.max(VIEW_PAD, window.innerWidth - width - VIEW_PAD));
  const spaceBelow = window.innerHeight - rect.bottom - VIEW_PAD;
  const spaceAbove = rect.top - VIEW_PAD;
  const openUp =
    placement === "up" || (placement === "auto" && spaceBelow < 160 && spaceAbove > spaceBelow);
  const available = (openUp ? spaceAbove : spaceBelow) - MENU_GAP;
  const maxHeight = Math.max(MENU_MIN_HEIGHT, Math.min(MENU_MAX_HEIGHT, available));
  if (openUp) {
    return {
      bottom: window.innerHeight - rect.top + MENU_GAP,
      left,
      width,
      maxHeight,
      placement: "up",
    };
  }
  return {
    top: rect.bottom + MENU_GAP,
    left,
    width,
    maxHeight,
    placement: "down",
  };
};

export const Select = ({
  value,
  onChange,
  options,
  placeholder,
  ariaLabel,
  id,
  disabled,
  placement = "auto",
  menuMinWidth = 0,
}: SelectProps): ReactNode => {
  const listId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLUListElement>(null);
  const [open, setOpen] = useState(false);
  const [menuPos, setMenuPos] = useState<MenuPos | null>(null);
  const [activeIndex, setActiveIndex] = useState(() =>
    Math.max(
      0,
      options.findIndex((option) => option.value === value),
    ),
  );

  const selected = options.find((option) => option.value === value);

  const close = (): void => {
    setOpen(false);
    triggerRef.current?.focus();
  };

  const openMenu = (): void => {
    if (disabled) return;
    const rect = triggerRef.current?.getBoundingClientRect();
    if (!rect) return;
    setMenuPos(menuPosFromRect(rect, placement, menuMinWidth));
    setActiveIndex(
      Math.max(
        0,
        options.findIndex((option) => option.value === value),
      ),
    );
    setOpen(true);
  };

  useEffect(() => {
    if (!open) return;

    const onPointer = (event: MouseEvent): void => {
      const target = event.target as Node | null;
      if (rootRef.current?.contains(target) || document.getElementById(listId)?.contains(target)) {
        return;
      }
      setOpen(false);
    };

    const onReposition = (): void => {
      const rect = triggerRef.current?.getBoundingClientRect();
      if (!rect) return;
      setMenuPos(menuPosFromRect(rect, placement, menuMinWidth));
    };

    const onKey = (event: globalThis.KeyboardEvent): void => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
      triggerRef.current?.focus();
    };

    window.addEventListener("mousedown", onPointer);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("resize", onReposition);
    window.addEventListener("scroll", onReposition, true);
    return () => {
      window.removeEventListener("mousedown", onPointer);
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("resize", onReposition);
      window.removeEventListener("scroll", onReposition, true);
    };
  }, [open, listId, placement, menuMinWidth]);

  useEffect(() => {
    if (!open) return;
    const item = menuRef.current?.querySelector<HTMLElement>('[data-active="true"]');
    item?.scrollIntoView({ block: "nearest" });
  }, [open, activeIndex]);

  const choose = (next: string): void => {
    onChange(next);
    close();
  };

  const onTriggerKey = (event: ReactKeyboardEvent<HTMLButtonElement>): void => {
    if (
      event.key === "ArrowDown" ||
      event.key === "ArrowUp" ||
      event.key === "Enter" ||
      event.key === " "
    ) {
      event.preventDefault();
      openMenu();
    }
  };

  const onListKey = (event: ReactKeyboardEvent<HTMLUListElement>): void => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      close();
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex((index) => Math.min(options.length - 1, index + 1));
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((index) => Math.max(0, index - 1));
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      const option = options[activeIndex];
      if (option) choose(option.value);
    }
  };

  return (
    <div ref={rootRef} className="select-root">
      <button
        ref={triggerRef}
        id={id}
        type="button"
        className="select"
        aria-label={ariaLabel}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        disabled={disabled}
        data-open={open}
        onClick={() => (open ? close() : openMenu())}
        onKeyDown={onTriggerKey}
      >
        <span className="select__value">{selected?.label ?? placeholder}</span>
        <span className="select__icon">
          <ChevronDown size={14} strokeWidth={1.5} />
        </span>
      </button>
      {open && menuPos && typeof document !== "undefined"
        ? createPortal(
            <ul
              id={listId}
              className="select-menu"
              role="listbox"
              tabIndex={-1}
              data-placement={menuPos.placement}
              style={{
                top: menuPos.top,
                bottom: menuPos.bottom,
                left: menuPos.left,
                width: menuPos.width,
                maxHeight: menuPos.maxHeight,
              }}
              onKeyDown={onListKey}
              ref={(node) => {
                menuRef.current = node;
                node?.focus();
              }}
            >
              {options.map((option, index) => {
                const isSelected = option.value === value;
                return (
                  <li
                    key={option.value}
                    role="option"
                    aria-selected={isSelected}
                    className="select-item"
                    data-active={index === activeIndex}
                    data-selected={isSelected}
                    onMouseEnter={() => setActiveIndex(index)}
                    onMouseDown={(event) => event.preventDefault()}
                    onClick={() => choose(option.value)}
                  >
                    <span className="select-item__indicator">
                      {isSelected ? <Check size={12} strokeWidth={2} /> : null}
                    </span>
                    {option.label}
                  </li>
                );
              })}
            </ul>,
            document.body,
          )
        : null}
    </div>
  );
};
