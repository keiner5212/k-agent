import { Loader2, Wand2 } from "lucide-react";
import type { ReactNode } from "react";
import { IconButton } from "@/components/IconButton";

type MagicGenerateButtonProps = {
  label: string;
  disabled?: boolean;
  loading?: boolean;
  onClick: () => void;
  className?: string;
};

export const MagicGenerateButton = ({
  label,
  disabled = false,
  loading = false,
  onClick,
  className,
}: MagicGenerateButtonProps): ReactNode => {
  const classes = className ? `magic-generate ${className}` : "magic-generate";

  return (
    <IconButton label={label} className={classes} onClick={onClick} disabled={disabled || loading}>
      {loading ? (
        <Loader2 size={16} strokeWidth={1.5} className="spin" />
      ) : (
        <Wand2 size={16} strokeWidth={1.5} />
      )}
    </IconButton>
  );
};
