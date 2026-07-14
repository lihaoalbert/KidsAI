# Phase 1 rename sed script — apply via: sed -i '' -f scripts/phase1.sed FILE
# 注意: 按 *最长前缀* 写在前 — 否则短前缀会先吃掉.

# ---- BRAND by prefix ----
# bg
s/\bbg-brand-50\b/bg-accent-soft/g
s/\bbg-brand-100\b/bg-accent-soft-2/g
s/\bbg-brand-200\b/bg-accent-line/g
s/\bbg-brand-300\b/bg-accent\/40/g
s/\bbg-brand-400\b/bg-accent\/40/g
s/\bbg-brand-500\b/bg-accent/g
s/\bbg-brand-600\b/bg-accent/g
s/\bbg-brand-700\b/bg-accent-hover/g
s/\bbg-brand-800\b/bg-accent-active/g
s/\bbg-brand-900\b/bg-accent-active/g
# file:
s/\bfile:bg-brand-100\b/file:bg-accent-soft-2/g
s/\bfile:bg-brand-200\b/file:bg-accent-line/g
s/\bfile:text-brand-700\b/file:text-accent-ink/g
s/\bfile:text-brand-800\b/file:text-accent-ink/g
s/\bhover:file:bg-brand-200\b/hover:file:bg-accent-line/g
s/\bhover:file:bg-brand-300\b/hover:file:bg-accent\/40/g
# text
s/\btext-brand-500\b/text-accent-ink/g
s/\btext-brand-600\b/text-accent-ink/g
s/\btext-brand-700\b/text-accent-ink/g
s/\btext-brand-800\b/text-accent-ink/g
s/\btext-brand-900\b/text-accent-ink/g
# border
s/\bborder-brand-200\b/border-accent-line/g
s/\bborder-brand-300\b/border-accent/g
s/\bborder-brand-400\b/border-accent/g
s/\bborder-brand-500\b/border-accent/g
# ring
s/\bring-brand-100\b/ring-accent\/40/g
s/\bring-brand-300\b/ring-accent\/40/g
s/\bring-brand-400\b/ring-accent\/40/g
# gradient
s/\bfrom-brand-500\b/from-accent/g
s/\bfrom-brand-600\b/from-accent/g
s/\bto-brand-500\b/to-accent/g
s/\bto-brand-600\b/to-accent/g
s/\bvia-brand-500\b/via-accent/g
# stateful prefixes
s/\bhover:bg-brand-50\b/hover:bg-accent-soft/g
s/\bhover:bg-brand-100\b/hover:bg-accent-soft-2/g
s/\bhover:bg-brand-200\b/hover:bg-accent-line/g
s/\bhover:bg-brand-300\b/hover:bg-accent\/40/g
s/\bhover:bg-brand-400\b/hover:bg-accent\/40/g
s/\bhover:bg-brand-500\b/hover:bg-accent/g
s/\bhover:bg-brand-600\b/hover:bg-accent/g
s/\bhover:bg-brand-700\b/hover:bg-accent-hover/g
s/\bhover:text-brand-500\b/hover:text-accent-ink/g
s/\bhover:text-brand-600\b/hover:text-accent-ink/g
s/\bhover:text-brand-700\b/hover:text-accent-ink/g
s/\bhover:text-brand-800\b/hover:text-accent-ink/g
s/\bhover:border-brand-300\b/hover:border-accent/g
s/\bhover:border-brand-400\b/hover:border-accent/g
s/\bhover:border-brand-500\b/hover:border-accent/g
s/\bactive:bg-brand-50\b/active:bg-accent-soft-2/g
s/\bactive:bg-brand-100\b/active:bg-accent-soft-2/g
s/\bactive:bg-brand-800\b/active:bg-accent-active/g
s/\bdisabled:bg-brand-300\b/disabled:bg-accent\/40/g
s/\bdisabled:bg-brand-400\b/disabled:bg-accent\/40/g
s/\bdisabled:bg-brand-500\b/disabled:bg-accent\/40/g
s/\bdisabled:bg-brand-600\b/disabled:bg-accent\/40/g
s/\bfocus:ring-brand-100\b/focus:ring-accent\/40/g
s/\bfocus:ring-brand-400\b/focus:ring-accent\/40/g
s/\bfocus:border-brand-500\b/focus:border-ac