#!/usr/bin/env bash
# Phase 1 真重命名 — Perl 一次性执行所有替换, 稳定可靠.
# 长前缀先跑 — 用最长 match 的 rule 在前即可.

set -euo pipefail
cd "$(dirname "$0")/.."

EXCLUDE_PATTERNS=(
  "src/components/Button.tsx"
  "src/components/Card.tsx"
  "src/components/Input.tsx"
  "src/components/Sidebar.tsx"
  "src/components/ui/ToastHost.tsx"
  "src/components/system/ModeBadge.tsx"
  "src/components/system/ModeSwitchDialog.tsx"
  "src/App.tsx"
  "src/styles/globals.css"
)

should_exclude() {
  local file="$1"
  for pat in "${EXCLUDE_PATTERNS[@]}"; do
    if [[ "$file" == *"$pat" ]]; then
      return 0
    fi
  done
  return 1
}

PERL_SCRIPT='
use strict;
use warnings;

# 顺序: 长前缀在前, 短前缀在后. 用 \b 保证 word boundary.
my @rules = (
  # === LONG PREFIX: stateful + file + gradient ===
  [ "hover:file:bg-brand-200", "hover:file:bg-accent-line" ],
  [ "hover:file:bg-brand-300", "hover:file:bg-accent/40" ],
  [ "file:text-brand-700",     "file:text-accent-ink" ],
  [ "file:text-brand-800",     "file:text-accent-ink" ],
  [ "file:bg-brand-100",       "file:bg-accent-soft-2" ],
  [ "file:bg-brand-200",       "file:bg-accent-line" ],
  [ "disabled:bg-brand-300",   "disabled:bg-accent/40" ],
  [ "disabled:bg-brand-400",   "disabled:bg-accent/40" ],
  [ "disabled:bg-brand-500",   "disabled:bg-accent/40" ],
  [ "disabled:bg-brand-600",   "disabled:bg-accent/40" ],
  [ "hover:bg-brand-50",       "hover:bg-accent-soft" ],
  [ "hover:bg-brand-100",      "hover:bg-accent-soft-2" ],
  [ "hover:bg-brand-200",      "hover:bg-accent-line" ],
  [ "hover:bg-brand-300",      "hover:bg-accent/40" ],
  [ "hover:bg-brand-400",      "hover:bg-accent/40" ],
  [ "hover:bg-brand-500",      "hover:bg-accent" ],
  [ "hover:bg-brand-600",      "hover:bg-accent" ],
  [ "hover:bg-brand-700",      "hover:bg-accent-hover" ],
  [ "hover:text-brand-500",    "hover:text-accent-ink" ],
  [ "hover:text-brand-600",    "hover:text-accent-ink" ],
  [ "hover:text-brand-700",    "hover:text-accent-ink" ],
  [ "hover:text-brand-800",    "hover:text-accent-ink" ],
  [ "hover:border-brand-300",  "hover:border-accent" ],
  [ "hover:border-brand-400",  "hover:border-accent" ],
  [ "hover:border-brand-500",  "hover:border-accent" ],
  [ "active:bg-brand-50",      "active:bg-accent-soft-2" ],
  [ "active:bg-brand-100",     "active:bg-accent-soft-2" ],
  [ "active:bg-brand-800",     "active:bg-accent-active" ],
  [ "focus:ring-brand-100",    "focus:ring-accent/40" ],
  [ "focus:ring-brand-400",    "focus:ring-accent/40" ],
  [ "focus:border-brand-400",  "focus:border-accent" ],
  [ "focus:border-brand-500",  "focus:border-accent" ],
  [ "from-brand-500",          "from-accent" ],
  [ "from-brand-600",          "from-accent" ],
  [ "to-brand-500",            "to-accent" ],
  [ "to-brand-600",            "to-accent" ],
  [ "via-brand-500",           "via-accent" ],

  # === SINGLE-TON PREFIX: bg/text/border/ring by attribute ===
  [ "ring-brand-100",          "ring-accent/40" ],
  [ "ring-brand-300",          "ring-accent/40" ],
  [ "ring-brand-400",          "ring-accent/40" ],
  [ "border-brand-200",        "border-accent-line" ],
  [ "border-brand-300",        "border-accent" ],
  [ "border-brand-400",        "border-accent" ],
  [ "border-brand-500",        "border-accent" ],
  [ "text-brand-500",          "text-accent-ink" ],
  [ "text-brand-600",          "text-accent-ink" ],
  [ "text-brand-700",          "text-accent-ink" ],
  [ "text-brand-800",          "text-accent-ink" ],
  [ "text-brand-900",          "text-accent-ink" ],
  [ "bg-brand-50",             "bg-accent-soft" ],
  [ "bg-brand-100",            "bg-accent-soft-2" ],
  [ "bg-brand-200",            "bg-accent-line" ],
  [ "bg-brand-300",            "bg-accent/40" ],
  [ "bg-brand-400",            "bg-accent/40" ],
  [ "bg-brand-500",            "bg-accent" ],
  [ "bg-brand-600",            "bg-accent" ],
  [ "bg-brand-700",            "bg-accent-hover" ],
  [ "bg-brand-800",            "bg-accent-active" ],
  [ "bg-brand-900",            "bg-accent-active" ],

  # === WARM ===
  [ "bg-warm-50/40",           "bg-bg/40" ],
  [ "bg-warm-50/60",           "bg-bg/60" ],
  [ "bg-warm-50",              "bg-bg" ],
  [ "bg-warm-100",             "bg-accent-soft" ],
  [ "bg-warm-500",             "bg-highlight" ],
  [ "text-warm-500",           "text-highlight" ],
  [ "text-warm-600",           "text-highlight" ],
  [ "from-warm-50",            "from-bg" ],
  [ "from-warm-100",           "from-accent-soft" ],
  [ "from-warm-500",           "from-highlight" ],
  [ "to-warm-50",              "to-bg" ],
  [ "to-warm-100",             "to-accent-soft" ],
  [ "to-warm-500",             "to-highlight" ],

  # === NEUTRAL (gray/white/black) ===
  [ "bg-white",                "bg-surface" ],
  [ "text-gray-900",           "text-ink" ],
  [ "text-gray-800",           "text-ink-2" ],
  [ "text-gray-700",           "text-ink-2" ],
  [ "text-gray-600",           "text-ink-2" ],
  [ "text-gray-500",           "text-ink-2" ],
  [ "text-gray-400",           "text-ink-3" ],
  [ "text-gray-300",           "text-ink-3" ],
  [ "text-gray-200",           "text-ink-3" ],
  [ "text-gray-100",           "text-ink-3" ],
  [ "placeholder-gray-400",    "placeholder-ink-3" ],
  [ "placeholder-gray-500",    "placeholder-ink-3" ],
  [ "placeholder:text-gray-400", "placeholder:text-ink-3" ],
  [ "placeholder:text-gray-500", "placeholder:text-ink-3" ],
  [ "bg-gray-50",              "bg-surface-2" ],
  [ "bg-gray-100",             "bg-surface-2" ],
  [ "bg-gray-200",             "bg-surface-2" ],
  [ "hover:bg-gray-50",        "hover:bg-surface-2" ],
  [ "hover:bg-gray-100",       "hover:bg-surface-2" ],
  [ "hover:bg-gray-200",       "hover:bg-surface-2" ],
  [ "border-gray-50",          "border-line" ],
  [ "border-gray-100",         "border-line" ],
  [ "border-gray-200",         "border-line" ],
  [ "border-gray-300",         "border-line" ],
  [ "border-gray-400",         "border-line" ],
  [ "disabled:bg-gray-100",    "disabled:bg-surface-2" ],
  [ "disabled:bg-gray-200",    "disabled:bg-surface-2" ],
  [ "disabled:bg-gray-300",    "disabled:bg-surface-2" ],
  [ "disabled:text-gray-300",  "disabled:text-ink-3" ],
  [ "disabled:text-gray-400",  "disabled:text-ink-3" ],
  [ "disabled:text-gray-500",  "disabled:text-ink-3" ],

  # === STATE COLORS ===
  # red → danger
  [ "bg-red-50",               "bg-danger-soft" ],
  [ "bg-red-100",              "bg-danger-soft" ],
  [ "bg-red-500",              "bg-danger" ],
  [ "bg-red-600",              "bg-danger" ],
  [ "bg-red-700",              "bg-danger/90" ],
  [ "text-red-500",            "text-danger" ],
  [ "text-red-600",            "text-danger" ],
  [ "text-red-700",            "text-danger" ],
  [ "border-red-200",          "border-danger-soft" ],
  [ "border-red-300",          "border-danger" ],
  [ "border-red-500",          "border-danger" ],
  [ "ring-red-200",            "ring-danger-soft" ],
  [ "hover:bg-red-50",         "hover:bg-danger-soft" ],
  [ "hover:bg-red-100",        "hover:bg-danger-soft" ],
  # amber → warning
  [ "bg-amber-50",             "bg-warning-soft" ],
  [ "bg-amber-100",            "bg-warning-soft" ],
  [ "bg-amber-500",            "bg-warning" ],
  [ "text-amber-500",          "text-warning" ],
  [ "text-amber-600",          "text-warning" ],
  [ "text-amber-700",          "text-warning" ],
  [ "text-amber-800",          "text-warning" ],
  [ "border-amber-200",        "border-warning-soft" ],
  [ "border-amber-300",        "border-warning-soft" ],
  # emerald → success
  [ "bg-emerald-50",           "bg-success-soft" ],
  [ "bg-emerald-100",          "bg-success-soft" ],
  [ "bg-emerald-500",          "bg-success" ],
  [ "bg-emerald-600",          "bg-success" ],
  [ "text-emerald-600",        "text-success" ],
  [ "text-emerald-700",        "text-success" ],
  # purple (BANNED per DESIGN.md §9.2) → accent
  [ "bg-purple-100",           "bg-accent-soft" ],
  [ "bg-purple-200",           "bg-accent-soft-2" ],
  [ "bg-purple-500",           "bg-accent" ],
  [ "bg-purple-600",           "bg-accent" ],
  [ "bg-purple-700",           "bg-accent-hover" ],
  [ "text-purple-600",         "text-accent-ink" ],
  [ "text-purple-700",         "text-accent-ink" ],
  [ "border-purple-200",       "border-accent-line" ],
  [ "border-purple-300",       "border-accent" ],
  [ "hover:bg-purple-200",     "hover:bg-accent-soft-2" ],
  [ "hover:bg-purple-700",     "hover:bg-accent-hover" ],
);

local $/;
my $content = <>;

for my $r (@rules) {
  my ($from, $to) = @$r;
  my $quoted = quotemeta $from;
  $content =~ s/\b$quoted\b/$to/g;
}

print $content;
'

CHANGED=()
for f in $(grep -rl "brand-\|warm-\|gray-\| bg-white\|text-gray\|border-gray\|placeholder-gray\|bg-red\|text-red\|border-red\|bg-amber\|text-amber\|bg-emerald\|text-emerald\|bg-purple\|text-purple\|border-purple\|file:bg-brand\|file:text-brand\|hover:file:bg-brand" src --include="*.tsx" --include="*.ts" 2>/dev/null | sort -u); do
  if should_exclude "$f"; then
    continue
  fi

  cp "$f" "$f.__phase1_bak"
  perl -i -e "$PERL_SCRIPT" "$f"

  if ! diff -q "$f" "$f.__phase1_bak" > /dev/null 2>&1; then
    CHANGED+=("$f")
  fi
  rm "$f.__phase1_bak"
done

echo ""
echo "=== 修改的文件 (${#CHANGED[@]} 个) ==="
printf '%s\n' "${CHANGED[@]}"
