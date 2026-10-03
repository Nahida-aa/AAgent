#!/usr/bin/env bash
# 扫描某个 rev 区间内 zed 上游的改动，筛出「命中我们本地 fork」的部分。
#
# AAgent 的 packages/* 里有 167 个 crate 是从 zed 整包搬过来的（path 依赖），
# 它们不会因为 Cargo.toml 里的 git rev 变化而自动更新 —— 那是 fork 同步（人工 port），
# 不是 rev 同步（sed 换 rev）。这个脚本只回答一个问题：
#
#   「<old>..<new> 之间，zed 改了哪些我们 fork 的 crate？」
#
# 用法：
#   tools/zed-fork-scan.sh                        # old=当前 pin, new=origin/main
#   tools/zed-fork-scan.sh <old> <new>            # 位置参数指定区间
#   tools/zed-fork-scan.sh --old <old> --new <new>
#   tools/zed-fork-scan.sh --commits              # 只列触及 fork 的提交（不分档）
#   tools/zed-fork-scan.sh --files                # 每个命中 crate 改了哪些文件
#   tools/zed-fork-scan.sh --all                  # 连未命中我们 fork 的上游目录一起列出
#
# 输出第一列是 crate 名，第二列是上游变动的文件数（降序）。
# 位置参数与 --old/--new 可混用，但同时给时 --old/--new 优先。

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ZD="${ZED_REPO:-$HOME/repos/ide_ls/learn_ls/zed}"
CARGO="$ROOT/Cargo.toml"

[ -d "$ZD/.git" ] || { echo "找不到 zed 克隆: $ZD（用 ZED_REPO=... 覆盖）" >&2; exit 1; }
[ -f "$CARGO" ] || { echo "找不到 $CARGO" >&2; exit 1; }

MODE="table"
OLD_OPT=""
NEW_OPT=""
POS=()
while [ $# -gt 0 ]; do
  case "$1" in
    --files|--commits|--all) MODE="${1#--}" ;;
    --old) OLD_OPT="${2:?--old 需要一个值}"; shift ;;
    --new) NEW_OPT="${2:?--new 需要一个值}"; shift ;;
    -*) echo "未知参数: $1" >&2; exit 2 ;;
    *) POS+=("$1") ;;
  esac
  shift
done

# old：默认取 Cargo.toml 里第一个 zed rev（所有 zed 依赖锁的是同一个）
OLD="${OLD_OPT:-${POS[0]:-}}"
if [ -z "$OLD" ]; then
  OLD="$(grep -oE 'zed-industries/zed", rev = "[0-9a-f]+' "$CARGO" | head -1 | grep -oE '[0-9a-f]{40}')"
  [ -n "$OLD" ] || { echo "无法从 Cargo.toml 推断当前 rev，请显式传 <old>" >&2; exit 1; }
fi
NEW="${NEW_OPT:-${POS[1]:-origin/main}}"
git -C "$ZD" rev-parse --verify "$OLD" >/dev/null 2>&1 || { echo "old 不是合法 rev: $OLD" >&2; exit 1; }
git -C "$ZD" rev-parse --verify "$NEW" >/dev/null 2>&1 || { echo "new 不是合法 rev: $NEW" >&2; exit 1; }

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# 1) 我们 fork 的 crate 名 = path 依赖的目录 basename
#    （注意用目录名而非依赖 key：aa_http_client -> packages/http_client -> crates/http_client）
grep -oE 'path = "packages/[a-z_0-9-]+"' "$CARGO" \
  | sed 's|.*packages/||; s|"||' | sort -u > "$tmp/fork.txt"

# 2) 上游 <old>..<new> 里 crates/ 下各子目录的变动文件数
git -C "$ZD" diff --numstat "$OLD" "$NEW" -- crates/ \
  | awk 'NF>=3 { split($3, a, "/"); if (a[2] != "") print a[2] }' \
  | sort | uniq -c \
  | awk '{ c=$1; $1=""; sub(/^ /, ""); print $0 "\t" c }' | sort > "$tmp/up.tsv"

join -t$'\t' "$tmp/fork.txt" "$tmp/up.tsv" > "$tmp/hit.tsv" || true

n_fork=$(wc -l < "$tmp/fork.txt")
n_hit=$(wc -l < "$tmp/hit.tsv")

echo "区间 $OLD..$NEW"
echo "本地 fork $(printf '%d' "$n_fork") 个 crate，命中 $n_hit 个"
echo

paths="$(cut -f1 "$tmp/hit.tsv" | sed 's|^|crates/|' | tr '\n' ' ')"

case "$MODE" in
  files)
    sort -t$'\t' -k2,2nr "$tmp/hit.tsv" | while IFS=$'\t' read -r crate n; do
      echo "── $crate  ($n 个文件)"
      git -C "$ZD" diff --stat "$OLD" "$NEW" -- "crates/$crate" | sed 's/^/  /'
    done
    ;;
  commits)
    # 不分档，全量列出触及 fork 的提交，按时间倒序
    git -C "$ZD" log --format='%cd %h %s' --date=short "$OLD..$NEW" -- $paths
    ;;
  all)
    echo "上游 crates/ 下全部有改动的目录（★ = 我们 fork 了它）:"
    awk -F'\t' 'NR==FNR{ h[$1]=1; next } { printf "%s %-28s %s\n", ($1 in h ? "★" : " "), $1, $2 }' \
      "$tmp/hit.tsv" "$tmp/up.tsv"
    ;;
  *)
    sort -t$'\t' -k2,2nr "$tmp/hit.tsv" | awk -F'\t' '{ printf "%5s  %s\n", $2, $1 }'
    echo
    echo "下一步：tools/zed-fork-scan.sh $OLD $NEW --commits   # 逐条分诊"
    echo "        tools/zed-fork-scan.sh $OLD $NEW --files     # 看具体文件"
    ;;
esac