// 占位 loading 组件 — Phase 2 共享组件库.
//
// DESIGN.md §4.5: "Skeletal loaders matching exact layout dimensions —
// no generic circular spinners." 双模式自动通过 :root[data-mode] 翻转.
//
// 用法:
//   <Skeleton variant="text" className="w-32" />          // 一行
//   <Skeleton variant="circle" className="w-10 h-10" />   // 圆形头像位
//   <Skeleton variant="card" />                            // 一张卡

import type { HTMLAttributes } from 'react';

export type SkeletonVariant = 'text' | 'circle' | 'card' | 'block';

interface SkeletonProps extends HTMLAttributes<HTMLDivElement> {
  variant?: SkeletonVariant;
  /** 行数 (仅 text 生效) */
  lines?: number;
}

const variantClasses: Record<SkeletonVariant, string> = {
  // 单行文本 — h-3 + rounded-sm
  text: 'h-3 rounded-sm',
  // 圆形 — 配合 w-N h-N 尺寸, 强制 1:1 圆
  circle: 'rounded-full',
  // 卡片占位 — 整块圆角
  card: 'h-32 rounded-card',
  // 通用块 — 留给自定义尺寸
  block: 'rounded-md',
};

export default function Skeleton({
  variant = 'text',
  lines = 1,
  className = '',
  ...rest
}: SkeletonProps) {
  // 暗色背景 token + 200ms 闪烁 — 用 transform/opacity 避免 layout thrash
  const base =
    'relative overflow-hidden bg-surface-2 ' +
    'before:absolute before:inset-0 ' +
    'before:bg-gradient-to-r before:from-transparent ' +
    'before:via-ink-3/10 before:to-transparent ' +
    'before:animate-shimmer ' +
    'motion-reduce:before:hidden';

  if (variant === 'text' && lines > 1) {
    return (
      <div className="space-y-2" data-testid="skeleton-text">
        {Array.from({ length: lines }).map((_, i) => (
          <div
            key={i}
            className={[
              base,
              variantClasses.text,
              // 最后一行短一点 — 视觉上更像真实段落
              i === lines - 1 ? 'w-2/3' : 'w-full',
            ].join(' ')}
            {...rest}
          />
        ))}
      </div>
    );
  }

  return (
    <div
      data-testid="skeleton"
      className={[base, variantClasses[variant], className].join(' ')}
      {...rest}
    />
  );
}
