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
import { GlassFill } from "@/lib/glass-warp";
import { GLASS } from "@/lib/glass";

type SelectOption = {
  value: string;
  label: ReactNode;
};

type SelectProps = {
  value: string;
  onChange: (next: string) => void;
  options: readonly SelectOption[];
  placeholder?: ReactNode;
  ariaLabel?: string;
  id?: string;
  disabled?: boolean;
};

type MenuPos = {
  top: number;
  left: number;
  width: number;
};

export const Select = ({
  value,
  onChange,
  options,
  placeholder,
  ariaLabel,
  id,
  disabled,
}: SelectProps): ReactNode => {
  const listId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
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
    setMenuPos({ top: rect.bottom + 4, left: rect.left, width: rect.width });
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
      setMenuPos({ top: rect.bottom + 4, left: rect.left, width: rect.width });
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
  }, [open, listId]);

  const choose = (next: string): void => {
    onChange(next);
    close();
  };

  const onTriggerKey = (event: ReactKeyboardEvent<HTMLButtonElement>): void => {
    if (event.key === "ArrowDown" || event.key === "Enter" || event.key === " ") {
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
        <GlassFill
          displacementScale={GLASS.select.displacementScale}
          aberrationIntensity={GLASS.select.aberrationIntensity}
        />
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
              style={{ top: menuPos.top, left: menuPos.left, width: menuPos.width }}
              onKeyDown={onListKey}
              ref={(node) => {
                node?.focus();
              }}
            >
              <GlassFill
                displacementScale={GLASS.menu.displacementScale}
                aberrationIntensity={GLASS.menu.aberrationIntensity}
              />
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
