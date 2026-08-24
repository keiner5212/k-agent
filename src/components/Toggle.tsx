import type { ChangeEvent, ReactNode } from "react";
import { useId } from "react";
import { useTranslation } from "react-i18next";
import { GlassFill } from "@/lib/glass-warp";
import { GLASS } from "@/lib/glass";

type ToggleProps = {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  description?: string;
  disabled?: boolean;
  showLabel?: boolean;
};

export const Toggle = ({
  checked,
  onChange,
  label,
  description,
  disabled,
  showLabel = true,
}: ToggleProps): ReactNode => {
  const id = useId();
  const { t } = useTranslation();
  const handleChange = (event: ChangeEvent<HTMLInputElement>): void => {
    onChange(event.target.checked);
  };

  return (
    <div className={showLabel ? "toggle-row" : "toggle-row toggle-row--control"}>
      <label className={showLabel ? "toggle-row__body" : "visually-hidden"} htmlFor={id}>
        <span className="toggle-row__label">{label}</span>
        {description ? <span className="toggle-row__hint">{description}</span> : null}
      </label>
      <span className="toggle" data-on={checked}>
        <input
          id={id}
          type="checkbox"
          role="switch"
          aria-checked={checked}
          className="toggle__input"
          checked={checked}
          onChange={handleChange}
          disabled={disabled}
        />
        <span aria-hidden="true" className="toggle__track">
          <GlassFill
            displacementScale={GLASS.toggle.displacementScale}
            aberrationIntensity={GLASS.toggle.aberrationIntensity}
          />
          <span className="toggle__thumb" />
        </span>
        <span className="visually-hidden">{checked ? t("common.on") : t("common.off")}</span>
      </span>
    </div>
  );
};
