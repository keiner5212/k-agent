import type { ChangeEvent, ReactNode } from "react";
import { useId } from "react";
import { useTranslation } from "react-i18next";

type ToggleProps = {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  description?: string;
  disabled?: boolean;
};

export const Toggle = ({
  checked,
  onChange,
  label,
  description,
  disabled,
}: ToggleProps): ReactNode => {
  const id = useId();
  const { t } = useTranslation();
  const handleChange = (event: ChangeEvent<HTMLInputElement>): void => {
    onChange(event.target.checked);
  };

  return (
    <div className="toggle-row">
      <label className="toggle-row__body" htmlFor={id}>
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
          <span className="toggle__thumb" />
        </span>
        <span className="visually-hidden">{checked ? t("common.on") : t("common.off")}</span>
      </span>
    </div>
  );
};
