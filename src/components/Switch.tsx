import { cls } from '../utils';

interface Props {
  checked: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
  label?: string;
}

export function Switch({ checked, onChange, disabled, label }: Props) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cls('switch', checked && 'switch-on', disabled && 'opacity-50')}
    >
      <span className="switch-knob" />
    </button>
  );
}
