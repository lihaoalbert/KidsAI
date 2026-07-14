import type { ButtonHTMLAttributes, ReactNode } from 'react';
import Skeleton from './ui/Skeleton';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
export type ButtonSize = 'sm' | 'md' | 'lg' | 'xl';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
  children: ReactNode;
}

// Mode-aware 自动通过 :root[data-mode] 翻转 — 不写条件分支
const variantClasses: Record<ButtonVariant, string> = {
  primary:
    'bg-accent text-bg hover:bg-accent-hover active:bg-accent-active disabled:bg-accent/40 disabled:text-bg/80',
  secondary:
    'bg-surface text-accent-ink border border-accent hover:bg-accent-soft active:bg-accent-soft-2',
  ghost:
    'bg-transparent text-accent-ink hover:bg-accent-soft active:bg-accent-soft-2',
  danger:
    'bg-danger text-bg hover:bg-danger/90 active:bg-danger/80 disabled:bg-danger/40',
};

const sizeClasses: Record<ButtonSize, string> = {
  sm: 'h-8 px-3 text-meta',
  md: 'h-10 px-4 text-sm',
  lg: 'h-12 px-5 text-base',
  xl: 'h-14 px-6 text-lg',
};

export default function Button({
  variant = 'primary',
  size = 'md',
  loading = false,
  disabled,
  children,
  className = '',
  ...rest
}: ButtonProps) {
  return (
    <button
      disabled={disabled || loading}
      className={[
        'inline-flex items-center justify-center gap-2 rounded-md font-medium transition-colors',
        'focus:outline-none focus:ring-2 focus:ring-accent/40 focus:ring-offset-1 focus:ring-offset-bg',
        'disabled:cursor-not-allowed disabled:opacity-70',
        variantClasses[variant],
        sizeClasses[size],
        className,
      ].join(' ')}
      {...rest}
    >
      {loading && (
        <Skeleton variant="block" className="h-3.5 w-12" />
      )}
      {children}
    </button>
  );
}