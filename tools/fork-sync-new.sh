#!/usr/bin/env bash
# 为一次 rev → rev 同步生成计划文件：.agents/fork-sync/<old7>_<new7>.md
#
# 为什么要单独一个文件：zed-port.md §16 是索引，长期同步会越来越长；
# 每次同步的细节（区间数据、工作流勾选、逐条 port 记录、踩的坑）各自成文，
# 便于他人/agent 查「现在在做什么、做过什么、为什么」。
#
# 用法：
#   tools/fork-sync-new.sh                        # old=当前 pin, new=origin/main
#   tools/fork-sync-new.sh <old> <new>            # 位置参数
#   tools/fork-sync-new.sh --old <old> --new <new>
#   tools/fork-sync-new.sh --list                 # 只列已有同步文件与状态
#   tools/fork-sync-new.sh --batch "批次 A" --note "先跑通流程"
#
# 已存在的文件不会被覆盖（会提示并退出），要继续填就在原文件上改。

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ZD="${ZED_REPO:-$HOME/repos/ide_ls/learn_ls/zed}"
CARGO="$ROOT/Cargo.toml"
OUTDIR="$ROOT/.agents/fork-sync"

[ -d "$ZD/.git" ] || { echo "找不到 zed 克隆: $ZD（用 ZED_REPO=... 覆盖）" >&2; exit 1; }

MODE="new"; OLD_OPT=""; NEW_OPT=""; BATCH=""; NOTE=""
POS=()
while [ $# -gt 0 ]; do
  case "$1" in
    --list) MODE="list" ;;
    --old) OLD_OPT="${2:?--old 需要一个值}"; shift ;;
    --new) NEW_OPT="${2:?--new 需要一个值}"; shift ;;
    --batch) BATCH="${2:?--batch 需要一个值}"; shift ;;
    --note) NOTE="${2:?--note 需要一个值}"; shift ;;
    -*) echo "未知参数: $1" >&2; exit 2 ;;
    *) POS+=("$1") ;;
  esac
  shift
done

if [ "$MODE" = "list" ]; then
  echo "同步计划（$OUTDIR）："
  if [ -d "$OUTDIR" ]; then
    for f in "$OUTDIR"/*.md; do
      [ -e "$f" ] || continue
      case "$(basename "$f")" in README.md) continue ;; esac
      # 跳过空文件 / 没有「# 同步」标题的残留（如手搓的占位文件）
      grep -q '^# 同步 ' "$f" 2>/dev/null || continue
      st=$(grep -m1 -oE '🟡[^|]*|🟢[^|]*|🔴[^|]*|⚪[^|]*' "$f" | sed 's/[[:space:]]*$//')
      rng=$(grep -m1 -oE '^# 同步 .*' "$f")
      printf "  %-28s %-10s %s\n" "$(basename "$f")" "${st:-?}" "$rng"
    done
  fi
  exit 0
fi

OLD="${OLD_OPT:-${POS[0]:-}}"
if [ -z "$OLD" ]; then
  OLD="$(grep -oE 'zed-industries/zed", rev = "[0-9a-f]+' "$CARGO" | head -1 | grep -oE '[0-9a-f]{40}')"
  [ -n "$OLD" ] || { echo "无法从 Cargo.toml 推断当前 rev，请显式传 <old>" >&2; exit 1; }
fi
NEW="${NEW_OPT:-${POS[1]:-origin/main}}"
git -C "$ZD" rev-parse --verify "$OLD" >/dev/null 2>&1 || { echo "old 不是合法 rev: $OLD" >&2; exit 1; }
git -C "$ZD" rev-parse --verify "$NEW" >/dev/null 2>&1 || { echo "new 不是合法 rev: $NEW" >&2; exit 1; }

OLD_FULL=$(git -C "$ZD" rev-parse "$OLD")
NEW_FULL=$(git -C "$ZD" rev-parse "$NEW")
O7=${OLD_FULL:0:7}; N7=${NEW_FULL:0:7}
FILE="$OUTDIR/${O7}_${N7}.md"

if [ -e "$FILE" ]; then
  echo "已存在，不覆盖：$FILE" >&2
  echo "继续填就在原文件上改；要另起一版用不同的 rev 区间。" >&2
  exit 1
fi

GPUI_PATHS=(crates/gpui crates/gpui_macros crates/gpui_platform crates/gpui_wgpu
            crates/gpui_util crates/gpui_tokio crates/gpui_shared_string
            crates/gpui_apple crates/collections)

N_TOTAL=$(git -C "$ZD" log --oneline "$OLD_FULL..$NEW_FULL" | wc -l)
N_GPUI=$(git -C "$ZD" log --oneline "$OLD_FULL..$NEW_FULL" -- "${GPUI_PATHS[@]}" | wc -l)
OLD_DATE=$(git -C "$ZD" log -1 --format=%cd --date=short "$OLD_FULL")
NEW_DATE=$(git -C "$ZD" log -1 --format=%cd --date=short "$NEW_FULL")
N_HIT=$(./tools/zed-fork-scan.sh --old "$OLD_FULL" --new "$NEW_FULL" 2>/dev/null | sed -n '2p' | grep -oE '命中 [0-9]+' | grep -oE '[0-9]+')

# 命中 fork 的 crate（按变动文件数降序）
mapfile -t HITS < <(./tools/zed-fork-scan.sh --old "$OLD_FULL" --new "$NEW_FULL" 2>/dev/null \
  | awk '$1 ~ /^[0-9]+$/ && NF>=2 {print $2}' | sort -u)
N_HIT=${#HITS[@]}

# 触及 fork 的提交 + 每个提交实际落在哪些命中 crate（一次 log --name-only 拿全，避免 N×M 次调用）
MAP="$ROOT/.agents/fork-sync/.map.$$"
git -C "$ZD" log --format='@@@%h %s' --name-only "$OLD_FULL..$NEW_FULL" \
  -- $(printf 'crates/%s ' "${HITS[@]}") > "$MAP" 2>/dev/null || true
trap 'rm -f "$CONFLICTS" "$MAP"' EXIT

# ⚠ 冲突预警 —— 分三级
#
# 关键：不能用「本地有没有提交碰过这个文件」判断。在copy-fork 里，**port 动作本身**
# 就会让每个被同步过的文件出现在 git log 里（见 0ee33ca 那次重构级对齐），
# 那样判会把「我们同步过」误当成「我们改过」，几乎全是误报。
#
# 正确判据：把本地文件与「上游 old 版本」比，看差异能否被**已知移植变换**解释。
#   L0 快进    本地 == 上游 old，干净照搬，同步直接取上游新版即可
#   L1 已知变换  差异全是 workspace-ify / src/x.rs→src/lib.rs / crate 改名等
#   L2 需人工   有解释不了的差异 —— 可能是有意的aacode 改动，必须逐个看
#
# 注意：L2 只能筛出「与上游 old 不同且不像机械变换」的文件，**判断不了意图**。
# 「我们故意删了这些测试」「这个偏离是设计而非疏漏」这类信息只有人知道，
# 所以 L2 一律标注需人工确认，不要自动当成冲突。
PORT_PAT='zed_actions|aacode_actions|aagent|aacode|zlog|a_log|zed_credentials_provider|ad_credentials_provider|version\.workspace|license\.workspace|publish\.workspace|\[lints\]|\[lib\]|\[\[bin\]\]|path = "src/|cargo-shear|doctest = false|version = "0\.1\.0"|license = "GPL|ignored = '

# Cargo.toml 走归一化：剔掉 workspace-ify 会改的元信息段，再把已知改名对齐，
# 之后还有差异才算真分歧（正常只剩依赖增删）
norm_manifest() {
  sed -E 's/^[<>] //' \
    | sed -E 's/\.(workspace)\b//g' \
    | grep -vE '^(version|license|publish|description|homepage|repository|rust-version|edition|publish|authors|categories|keywords|readme|versioned_file_path)[[:space:]]*=' \
    | grep -vE '^\[(lints|lib|bin|\[\[bin\]\]|package\.metadata\.cargo-shear|package\.metadata\.component|package\.metadata\.rzup)\][[:space:]]*$' \
    | grep -vE '^(path|doctest|name|ignored|harness|crate-type)[[:space:]]*=' \
    | grep -vE '^#' \
    | grep -vE '^[[:space:]]*$' \
    | sed -E 's/\bzed_actions\b/aacode_actions/g; s/\bzlog\b/a_log/g; s/\bzed_credentials_provider\b/ad_credentials_provider/g; s/\bzed_resource_manager\b/a_resource_manager/g'
}

unexplained_of() {
  local up="$1" localf="$2"
  case "$up" in
    */Cargo.toml)
      { diff <(norm_manifest < /tmp/opencode/.upstream_file) \
           <(norm_manifest < "$ROOT/$localf") || true; } \
        | { grep -E '^[<>]' || true; } | wc -l ;;
    *)
      { diff /tmp/opencode/.upstream_file "$ROOT/$localf" || true; } \
        | { grep -E '^[<>]' || true; } \
        | { sed -E 's/^[<>] //' | grep -vE "$PORT_PAT" || true; } | wc -l ;;
  esac
}

CONFLICTS="$ROOT/.agents/fork-sync/.conflicts.$$"
: > "$CONFLICTS"
L0=0; L1=0; L2=0
for c in "${HITS[@]}"; do
  while IFS= read -r up; do
    rel="${up#crates/$c/}"
    [ "$rel" = "$up" ] && continue           # 不是 crates/<c>/ 下的
    local_path="packages/$c/$rel"
    git -C "$ZD" show "$OLD_FULL:$up" > /tmp/opencode/.upstream_file 2>/dev/null || continue
    # 本地文件不存在 = 上游新加的文件，直接照搬，无冲突
    [ -f "$ROOT/$local_path" ] || { echo -e "$local_path\t$up\tL0\t0\t" >> "$CONFLICTS"; L0=$((L0+1)); continue; }
    if diff -q /tmp/opencode/.upstream_file "$ROOT/$local_path" >/dev/null 2>&1; then
      echo -e "$local_path\t$up\tL0\t0\t" >> "$CONFLICTS"; L0=$((L0+1)); continue
    fi
    # 有差异：把能被已知移植变换解释的剔掉，剩下的就是未解释差异
    # 注意 set -e -o pipefail：diff 文件不同时返回 1，会连带整条管道失败，必须逐段 || true
    unexplained=$(unexplained_of "$up" "$local_path")
    if [ "$unexplained" -eq 0 ]; then lvl=L1; else lvl=L2; fi
    case $lvl in L1) L1=$((L1+1));; L2) L2=$((L2+1));; esac
    if [ "$lvl" = L2 ]; then
      case "$up" in
        */Cargo.toml)
          sample=$( { diff <(norm_manifest < /tmp/opencode/.upstream_file) \
                        <(norm_manifest < "$ROOT/$local_path") || true; } \
                   | { grep -E '^[<>]' || true; } | head -3 | tr '\n' ';' | cut -c1-160) ;;
        *)
          sample=$( { diff /tmp/opencode/.upstream_file "$ROOT/$local_path" || true; } \
                   | { grep -E '^[<>]' || true; } \
                   | { sed -E 's/^[<>] //' | grep -vE "$PORT_PAT" || true; } \
                   | head -3 | tr '\n' ';' | cut -c1-160) ;;
      esac
    else
      sample=
    fi
    echo -e "$local_path\t$up\t$lvl\t$unexplained\t$sample" >> "$CONFLICTS"
  done < <(git -C "$ZD" diff --name-only "$OLD_FULL" "$NEW_FULL" -- "crates/$c")
done
rm -f /tmp/opencode/.upstream_file
N_CONFLICT=$L2

mkdir -p "$OUTDIR"
TODAY=$(date +%F)

{
cat <<EOF
# 同步 ${O7} → ${N7}${BATCH:+（$BATCH）}

| | |
| --- | --- |
| 状态 | 🟡 计划中 |
| from | \`${OLD_FULL}\`（${OLD_DATE}） |
| to | \`${NEW_FULL}\`（${NEW_DATE}） |
| 区间提交数 | ${N_TOTAL} |
| gpui 命中提交 | ${N_GPUI} |
| fork 命中 | ${N_HIT} 个 crate |
| ⚠ 需人工调和（L2） | ${N_CONFLICT} |
| 负责人 | — |
| 创建 | ${TODAY} |

## 1. 为什么做这一批

${NOTE:-<!-- 为什么切在这里、解决什么问题。批次 A 的目的是跑通流程，不是吃到多少代码。 -->}

## 2. 目标 / 非目标

目标：

-

非目标：

-

## 3. 工作流

rev 同步是原子的（换 rev 会带进区间内全部改动），所以 W1/W2 必须同批次推；
W3 与它们无依赖，可按 crate 多人并行。

### W1 gpui_learn（必须先于 W2）

- [ ] W1.1 根 \`Cargo.toml\` 10 处 rev → \`${N7}\`
- [ ] W1.2 vendored 平台层适配（\`gpui-android\` 等）
- [ ] W1.3 commit + push，记下 gpui_learn 新 sha

### W2 aacode rev + gpui 适配

- [ ] W2.1 根 \`Cargo.toml\` 29 处 zed rev + \`Cargo.toml:536\` 的 \`ui\` rev
- [ ] W2.2 gpui API 适配（\`Platform\` trait 新成员等）
- [ ] W2.3 \`cargo check -p app -p workspace -p ui\` 过
- [ ] W2.4 \`cargo check --workspace\` 过（下限）
- [ ] W2.5 commit + push

### W3 fork 同步（按 crate 认领，一 crate 一 owner）

命中 ${N_HIT} 个 crate：

EOF
printf '%s\n' "${HITS[@]}" | sed 's/^/- [ ] /; s/$/（待认领）/' | sed 's/^/  /'

cat <<EOF

逐条 port 记录（一个上游 sha 一个 commit，便于单独 revert）：

| 上游 sha | crate | aacode 改过该文件? | 动作 | port commit | 认领 |
| --- | --- | --- | --- | --- | --- |
EOF

if grep -q '^@@@' "$MAP"; then
  python3 - "$MAP" <<'PY'
import sys, re
sha = None; subj = ''; seen = []
out = []
for line in open(sys.argv[1], encoding='utf-8'):
    line = line.rstrip('\n')
    if line.startswith('@@@'):
        if sha: out.append((sha, subj, seen))
        rest = line[3:].split(' ', 1)
        sha = rest[0]; subj = rest[1] if len(rest) > 1 else ''
        seen = []
        continue
    if line.strip():
        parts = line.split('/')
        if len(parts) > 1 and parts[1] and parts[1] not in seen:
            seen.append(parts[1])
if sha: out.append((sha, subj, seen))
for s, t, cs in out:
    print(f"| `{s}` | {', '.join(f'`{c}`' for c in cs) or '—'} | ? | {t} | | |")
PY
else
  echo '| — | — | — | 区间内没有触及 fork 的提交 | | |'
fi

cat <<EOF

## 4. ⚠ 冲突预警（按「差异能否被已知移植变换解释」分级）

**不要用「本地有没有提交碰过这个文件」判断冲突** —— copy-fork 里port 动作本身就会让每个
被同步过的文件出现在 \`git log\` 里，那样判几乎全是误报。本节用的是：把本地文件与
**上游 old 版本**比，看差异能否被已知变换（workspace-ify / \`src/x.rs\`→\`src/lib.rs\` /
\`zed_actions\`→\`aacode_actions\` / \`zlog\`→\`a_log\` 等）解释。

| 级别 | 含义 | 本批 |
| --- | --- | --- |
| **L0 快进** | 本地 == 上游 old，干净照搬 | ${L0} |
| **L1 已知变换** | 差异全是机械移植变换，按例应用即可 | ${L1} |
| **L2 需人工** | 有解释不了的差异 | ${L2} |

L2 只能筛出「与上游 old 不同、且不像机械变换」的文件，**判断不了意图**。
「我们故意删了这些测试」「这个偏离是设计而非疏漏」只有人知道，所以 L2 一律人工确认，
不要自动当成冲突。

| 本地路径 | 上游路径 | 级别 | 未解释行 | 样例 |
| --- | --- | --- | --- | --- |
EOF

awk -F'\t' '{printf "| `%s` | `%s` | **%s** | %s | %s |\n", $1, $2, $3, $4, ($5==""?"—":$5)}' "$CONFLICTS"

cat <<EOF

### L2 逐个确认

\`\`\`sh
git log --oneline -- <本地路径>                # aacode 侧改动史
git -C $ZD diff $OLD_FULL..$NEW_FULL -- <上游路径>   # 上游这次改了什么
diff <(git -C $ZD show $OLD_FULL:<上游路径>) <本地路径>   # 当前分歧点
\`\`\`
EOF

cat <<EOF

## 5. 验收

- [ ] \`cargo check -p app -p workspace -p ui\`
- [ ] \`cargo check --workspace\`
- [ ] 每条 fork 同步都确认过「该文件 aacode 自己没改动」，有的已在 commit 里说明
- [ ] gpui_learn 侧的 Web/Android 示例仍能 build
- [ ] 回来更新 zed-port.md §16 索引与本文件状态

## 6. 踩的坑

<!-- 做的时候随手记，这里是后来人最需要的东西 -->

## 7. 遗留 / 下一批

-
EOF
} > "$FILE"

echo "已生成 $FILE"
echo
sed -n '2,14p' "$FILE"