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
        <label className="block text-sm font-medium text-gray-800 mb-1.5">
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
          'bg-white border',
          isError
            ? 'border-red-500 focus:border-red-500 focus:ring-2 focus:ring-red-200'
            : 'border-gray-300 focus:border-brand-500 focus:ring-2 focus:ring-brand-100',
          'focus:outline-none',
          'placeholder:text-gray-400',
          'disabled:bg-gray-50 disabled:text-gray-500',
          className,
        ].join(' ')}
        {...rest}
      />
      <div className="mt-1.5 flex justify-between text-xs">
        <span className={isError ? 'text-red-600' : 'text-gray-500'}>
          {errorMessage || helperText || ''}
        </span>
        {showCount && maxLength && (
          <span className="text-gray-400">
            {current.length} / {maxLength}
          </span>
        )}
      </div>
    </div>
  );
}
