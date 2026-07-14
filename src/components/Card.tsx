import type { HTMLAttributes, ReactNode } from 'react';

export type CardVariant = 'default' | 'elevated' | 'filled' | 'bordered';

interface CardProps extends Omit<HTMLAttributes<HTMLDivElement>, 'title'> {
  variant?: CardVariant;
  cover?: ReactNode;
  title?: ReactNode;
  description?: ReactNode;
  footer?: ReactNode;
  children?: ReactNode;
}

// Mode-aware 自动通过 :root[data-mode] 翻转
const variantClasses: Record<CardVariant, string> = {
  default: 'bg-surface border border-line',
  elevated: 'bg-surface shadow-md border border-transparent',
  filled: 'bg-accent-soft border border-transparent',
  bordered: 'bg-surface border-2 border-accent-line',
};

export default function Card({
  variant = 'default',
  cover,
  title,
  description,
  footer,
  children,
  className = '',
  ...rest
}: CardProps) {
  return (
    <div
      className={[
        'rounded-card overflow-hidden text-ink',
        variantClasses[variant],
        className,
      ].join(' ')}
      {...rest}
    >
      {cover && <div className="w-full aspect-video bg-surface-2">{cover}</div>}
      <div className="p-4">
        {title && (
          <h3 className="text-base font-semibold text-ink mb-1">
            {title}
          </h3>
        )}
        {description && (
          <p className="text-sm text-ink-2 line-clamp-2 mb-2">
            {description}
          </p>
        )}
        {children}
      </div>
      {footer && (
        <div className="px-4 py-3 border-t border-line bg-surface-2">
          {footer}
        </div>
      )}
    </div>
  );
}