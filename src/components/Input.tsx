import type { InputHTMLAttributes } from 'react';
import { useState } from 'react';

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  helperText?: string;
  errorMessage?: string;
  showCount?: boolean;
  maxLength?: number;
}

export default function Input({
  label,
  helperText,
  errorMessage,
  showCount = false,
  maxLength,
  value,
  defaultValue,
  onChange,
  className = '',
  ...rest
}: InputProps) {
  const [internal, setInternal] = useState<string>(
    String(defaultValue ?? value ?? ''),
  );
  const current = String(value ?? internal);
  const isError = !!errorMessage;

  return (
    <div className="w-full">
      {label && (
        <label className="block text-sm font-medium text-ink mb-1.5">
          {label}
        </label>
      )}
      <input
        value={value}
        defaultValue={defaultValue}
        maxLength={maxLength}
        onChange={(e) => {
          setInternal(e.target.value);
          onChange?.(e);
        }}
        className={[
          'w-full h-10 px-3 rounded-md text-sm transition-colors',
          'bg-surface border text-ink',
          isError
            ? 'border-danger focus:border-danger focus:ring-2 focus:ring-danger-soft'
            : 'border-line focus:border-accent focus:ring-2 focus:ring-accent-soft',
          'focus:outline-none',
          'placeholder:text-ink-3',
          'disabled:bg-surface-2 disabled:text-ink-3',
          className,
        ].join(' ')}
        {...rest}
      />
      <div className="mt-1.5 flex justify-between text-xs">
        <span className={isError ? 'text-danger' : 'text-ink-2'}>
          {errorMessage || helperText || ''}
        </span>
        {showCount && maxLength && (
          <span className="text-ink-3">
            {current.length} / {maxLength}
          </span>
        )}
      </div>
    </div>
  );
}