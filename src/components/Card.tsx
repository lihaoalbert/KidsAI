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

const variantClasses: Record<CardVariant, string> = {
  default: 'bg-white border border-gray-200',
  elevated: 'bg-white shadow-md border border-transparent',
  filled: 'bg-brand-50 border border-transparent',
  bordered: 'bg-white border-2 border-brand-200',
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
        'rounded-lg overflow-hidden',
        variantClasses[variant],
        className,
      ].join(' ')}
      {...rest}
    >
      {cover && <div className="w-full aspect-video bg-gray-100">{cover}</div>}
      <div className="p-4">
        {title && (
          <h3 className="text-base font-semibold text-gray-900 mb-1">
            {title}
          </h3>
        )}
        {description && (
          <p className="text-sm text-gray-600 line-clamp-2 mb-2">
            {description}
          </p>
        )}
        {children}
      </div>
      {footer && (
        <div className="px-4 py-3 border-t border-gray-100 bg-gray-50">
          {footer}
        </div>
      )}
    </div>
  );
}
