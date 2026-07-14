/** @type {import('tailwindcss').Config} */

// 语义色 token — 实际值在 globals.css 的 :root[data-mode="child|adult"] 里按模式翻转。
// 用 rgb(var(--x) / <alpha-value>) 形式以保留 Tailwind 透明度工具类 (如 bg-accent/40)。
const token = (name) => `rgb(var(${name}) / <alpha-value>)`;

export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        // 中性
        bg: token('--c-bg'),
        surface: token('--c-surface'),
        'surface-2': token('--c-surface-2'),
        ink: token('--c-ink'),
        'ink-2': token('--c-ink-2'),
        'ink-3': token('--c-ink-3'),
        line: token('--c-line'),
        // 主色 (单 accent)
        accent: token('--c-accent'),
        'accent-hover': token('--c-accent-hover'),
        'accent-active': token('--c-accent-active'),
        'accent-soft': token('--c-accent-soft'),
        'accent-soft-2': token('--c-accent-soft-2'),
        'accent-line': token('--c-accent-line'),
        'accent-ink': token('--c-accent-ink'),
        // 点缀 + 状态
        highlight: token('--c-highlight'),
        danger: token('--c-danger'),
        'danger-soft': token('--c-danger-soft'),
        warning: token('--c-warning'),
        'warning-soft': token('--c-warning-soft'),
        success: token('--c-success'),
        'success-soft': token('--c-success-soft'),
        // 暗色背景 (代码块 / 暗预览 — 两模式都一致)
        code: token('--c-code-bg'),
        'code-ink': token('--c-code-ink'),
      },
      fontFamily: {
        display: ['Geist', '"PingFang SC"', '"Source Han Sans SC"', '"Microsoft YaHei"', 'sans-serif'],
        sans: ['Geist', '"PingFang SC"', '"Source Han Sans SC"', '"Microsoft YaHei"', 'sans-serif'],
        mono: ['"Geist Mono"', '"JetBrains Mono"', 'Menlo', 'Monaco', 'monospace'],
      },
      fontSize: {
        // DESIGN.md §3.2 — 14px 是硬底线; 以下 <14px 仅供阶段5 收编前的过渡
        micro: ['10px', '1.4'],
        '2xs': ['11px', '1.45'],
        meta: ['0.8125rem', '1.5'],
      },
      borderRadius: {
        sm: '4px',
        md: '8px',
        lg: '12px',
        xl: '16px',
        '2xl': '24px',
        card: 'var(--r-card)',
      },
    },
  },
  plugins: [],
};
