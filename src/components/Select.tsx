import { Check, ChevronDown } from "lucide-react";
import {
  Root,
  Trigger,
  Value,
  Icon,
  Portal,
  Content,
  Viewport,
  Item,
  ItemIndicator,
  ItemText,
} from "@radix-ui/react-select";
import type { ReactNode } from "react";

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

export const Select = ({
  value,
  onChange,
  options,
  placeholder,
  ariaLabel,
  id,
  disabled,
}: SelectProps): ReactNode => {
  const selected = options.find((option) => option.value === value);

  return (
    <Root value={value} onValueChange={onChange} disabled={disabled}>
      <Trigger id={id} aria-label={ariaLabel} className="select">
        <Value placeholder={placeholder}>
          <span className="select__value">{selected?.label ?? placeholder}</span>
        </Value>
        <Icon className="select__icon">
          <ChevronDown size={14} strokeWidth={1.5} />
        </Icon>
      </Trigger>
      <Portal>
        <Content className="select-content" position="popper" sideOffset={6}>
          <Viewport className="select-viewport">
            {options.map((option) => (
              <Item key={option.value} value={option.value} className="select-item">
                <ItemIndicator className="select-item__indicator">
                  <Check size={12} strokeWidth={2} />
                </ItemIndicator>
                <ItemText>{option.label}</ItemText>
              </Item>
            ))}
          </Viewport>
        </Content>
      </Portal>
    </Root>
  );
};
