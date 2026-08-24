import { forwardRef, type ButtonHTMLAttributes, type MouseEvent } from "react";

type GlassButtonVariant = "primary" | "ghost" | "danger";

type GlassButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: GlassButtonVariant;
};

const variantClass: Record<GlassButtonVariant, string> = {
  primary: "btn--primary",
  ghost: "btn--ghost",
  danger: "btn--danger",
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
    const handleClick = (event: MouseEvent<HTMLButtonElement>): void => {
      if (disabled) return;
      onClick?.(event);
    };

    const classes = ["btn", variantClass[variant], className].filter(Boolean).join(" ");

    return (
      <button
        {...rest}
        ref={ref}
        type={type}
        className={classes}
        style={style}
        disabled={disabled}
        onClick={handleClick}
      >
        <span className="btn__label">{children}</span>
      </button>
    );
  },
);

GlassButton.displayName = "GlassButton";
