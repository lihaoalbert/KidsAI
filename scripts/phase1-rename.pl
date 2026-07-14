#!/usr/bin/perl
# Phase 1 rename — 真重命名 brand-/warm-/gray-/red-/amber-/emerald-/purple- → 语义 token.
# 用 Perl 是因为 sed 单行 \b word-boundary 在 BSD sed 上不稳定, Perl 内置 \b.
# 用法: perl -i scripts/phase1-rename.pl FILE [FILE ...]

use strict;
use warnings;

# 顺序很重要 — 长前缀在前 (e.g. text-brand-700 必须在 text-brand- 之前).
my @rules = (
  # ---- BRAND: bg ----
  [ 'bg-brand-50',     'bg-accent-soft' ],
  [ 'bg-brand-100',    'bg-accent-soft-2' ],
  [ 'bg-brand-200',    'bg-accent-line' ],
  [ 'bg-brand-300',    'bg-accent/40' ],
  [ 'bg-brand-400',    'bg-accent/40' ],
  [ 'bg-brand-500',    'bg-accent' ],
  [ 'bg-brand-600',    'bg-accent' ],
  [ 'bg-brand-700',    'bg-accent-hover' ],
  [ 'bg-brand-800',    'bg-accent-active' ],
  [ 'bg-brand-900',    'bg-accent-active' ],
  # file:
  [ 'file:bg-brand-100', 'file:bg-accent-soft-2' ],
  [ 'file:bg-brand-200', 'file:bg-accent-line' ],
  [ 'file:text-brand-700', 'file:text-accent-ink' ],
  [ 'file:text-brand-800', 'file:text-accent-ink' ],
  [ 'hover:file:bg-brand-200', 'hover:file:bg-accent-line' ],
  [ 'hover:file:bg-brand-300', 'hover:file:bg-accent/40' ],
  # text
  [ 'text-brand-500',  'text-accent-ink' ],
  [ 'text-brand-600',  'text-accent-ink' ],
  [ 'text-brand-700',  'text-accent-ink' ],
  [ 'text-brand-800',  'text-accent-ink' ],
  [ 'text-brand-900',  'text-accent-ink' ],
  # border
  [ 'border-brand-200', 'border-accent-line' ],
  [ 'border-brand-300', 'border-accent' ],
  [ 'border-brand-400', 'border-accent' ],
  [ 'border-brand-500', 'border-accent' ],
  # ring
  [ 'ring-brand-100',  'ring-accent/40' ],
  [ 'ring-brand-300',  'ring-accent/40' ],
  [ 'ring-brand-400',  'ring-accent/40' ],
  # gradient
  [ 'from-brand-500',  'from-accent' ],
  [ 'from-brand-600',  'from-accent' ],
  [ 'to-brand-500',    'to-accent' ],
  [ 'to-brand-600',    'to-accent' ],
  [ 'via-brand-500',   'via-accent' ],
  #