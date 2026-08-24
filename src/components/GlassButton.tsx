import LiquidGlass from "liquid-glass-react";
import {
  forwardRef,
  useRef,
  type ButtonHTMLAttributes,
  type MouseEvent,
} from "react";
import { GLASS_BUTTON, GLASS_LAYER_STYLE } from "@/lib/glass";
import { useSettingsStore } from "@/lib/settings";

type GlassButtonVariant = "primary" | "ghost" | "danger";

type GlassButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: GlassButtonVariant;
};

const variantClass: Record<GlassButtonVariant, string> = {
  primary: "btn--primary",
  ghost: "btn--ghost",
  danger: "btn--danger",
};

const variantTone: Record<GlassButtonVariant, string> = {
  primary: "glass-btn--primary",
  ghost: "glass-btn--ghost",
  danger: "glass-btn--danger",
};

export const GlassButton = forwardRef<HTMLButtonElement, GlassButtonProps>(
  (
    {
      children,
      variant = "primary",
      className,
      style,
      disabled,
      type = "button",
      onClick,
      ...rest
    },
    ref,
  ) => {
    const glass = useSettingsStore((state) => state.translucencyEnabled);
    const theme = useSettingsStore((state) => state.theme);
    const hostRef = useRef<HTMLElement | null>(null);

    const assignRef = (node: HTMLButtonElement | null): void => {
      hostRef.current = node;
      if (typeof ref === "function") ref(node);
      else if (ref) ref.current = node;
    };

    const handleClick = (event: MouseEvent<HTMLButtonElement>): void => {
      if (disabled) return;
      onClick?.(event);
    };

    if (!glass) {
      const classes = ["btn", variantClass[variant], className].filter(Boolean).join(" ");
      return (
        <button
          {...rest}
          ref={assignRef}
          type={type}
          className={classes}
          style={style}
          disabled={disabled}
          onClick={handleClick}
        >
          {children}
        </button>
      );
    }

    const classes = ["glass-btn", variantTone[variant], className].filter(Boolean).join(" ");

    return (
      <button
        {...rest}
        ref={assignRef}
        type={type}
        className={classes}
        style={style}
        disabled={disabled}
        onClick={handleClick}
      >
        <span className="glass-btn__sizer" aria-hidden="true">
          {children}
        </span>
        <span className="glass-btn__fx" aria-hidden="true">
          <LiquidGlass
            displacementScale={GLASS_BUTTON.displacementScale}
            blurAmount={GLASS_BUTTON.blurAmount}
            saturation={GLASS_BUTTON.saturation}
            aberrationIntensity={GLASS_BUTTON.aberrationIntensity}
            elasticity={disabled ? 0 : 0.12}
            cornerRadius={GLASS_BUTTON.cornerRadius}
            padding={GLASS_BUTTON.padding}
            overLight={theme === "light"}
            mouseContainer={hostRef}
            style={GLASS_LAYER_STYLE}
          >
            <span className="glass-btn__fill" />
          </LiquidGlass>
        </span>
        <span className="glass-btn__label">{children}</span>
      </button>
    );
  },
);

GlassButton.displayName = "GlassButton";
