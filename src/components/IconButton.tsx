import { forwardRef, type ButtonHTMLAttributes } from "react";

type IconButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  label: string;
};

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(
  ({ label, className, type = "button", children, ...rest }, ref) => {
    const classes = className ? `icon-button ${className}` : "icon-button";

    return (
      <button
        {...rest}
        ref={ref}
        type={type}
        className={classes}
        aria-label={label}
        title={label}
      >
        {children}
      </button>
    );
  },
);

IconButton.displayName = "IconButton";
