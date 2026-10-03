run-app:
    cargo run -p aa-app

# 清理
clean:
    cargo clean

stats:
    scc . --exclude-dir node_modules,dist,build,target,venv,.venv,__pycache__,.git,vendor,out,cmake-build-debug,CMakeFiles --exclude-ext lock,json,md,yaml,yml,toml,ini,conf

# tools
## outline —— 打印源文件的符号大纲（zed 的 tree-sitter outline.scm）
## 用法：just outline <文件...> [fields=...]
##   fields 留空 = 最少内容(text,line)；all = 全部；也可指定：text,line,relations
##   注意必须 --release 构建（grammars 的查询文件只在 release 下内嵌）
outline file='tools/outline/src/main.rs' fields='text,line':
    cargo run --release -p outline -- {{file}} --fields {{fields}}

## outline-json —— JSON 输出（嵌套 children 结构）
outline-json file='tools/outline/src/main.rs' fields='all':
    cargo run --release -p outline -- -f json --fields {{fields}} {{file}}

## outline-page —— 大文件翻页看（text 保持人读格式）
outline-page file='tools/outline/src/main.rs' offset='0' limit='30':
    cargo run --release -p outline -- -f text --fields text,line --offset {{offset}} --limit {{limit}} {{file}}

## gh-releases —— 查 GitHub release 列表 + 每个 tag 对应的 commit sha（单次 GraphQL 请求）
## 用法：just gh-releases <repo> [limit] [offset] [latest] [json]
##   repo    例：zed-industries/zed
##   limit   最多几条（默认 15）      offset 跳过前几条（默认 0）
##   latest  true 时只输出最新稳定版那一行   json  true 时输出原始 JSON
##   注意：just 的 recipe 参数只按位置传，不支持 name=value（--set 仅对变量生效）
##   例：just gh-releases zed-industries/zed 5
##      just gh-releases zed-industries/zed 1 0 true | cut -f2   # 只要最新稳定版的 sha
gh-releases repo limit='15' offset='0' latest='false' json='false':
    bun tools/gh-releases.ts {{repo}} --limit {{limit}} --offset {{offset}} {{ if latest == 'true' { '--latest' } else { '' } }} {{ if json == 'true' { '--json' } else { '' } }}

## zed-fork-scan —— 扫 rev 区间内 zed 上游改动，筛出命中我们本地 fork 的部分
## 用来驱动「fork 同步」（把上游改动 port 进 packages/*），区别于「rev 同步」（换 git rev）
##
## 四个 recipe 对应脚本的四种模式（just 的 --set 只能覆盖全局变量，改不了 recipe 参数，
## 所以这里用独立 recipe 而不是 --set mode=xxx）：
##   zed-fork-scan     概览表：命中了哪些 fork crate，各改了几个文件（降序）
##   zed-fork-commits 只列触及 fork 的提交，按时间倒序，用来逐条分诊
##   zed-fork-files    每个命中 crate 具体改了哪些文件
##   zed-fork-all      上游 crates/ 下全部有改动的目录，★ 标出我们 fork 了哪些
##
## 都接 [old] [new] 两个可选位置参数：old 省略时自动取根 Cargo.toml 里当前 pin 的那个，
## new 默认 origin/main，想跟 stable 就显式写 v1.22.0
_fork-scan mode old='' new='':
    tools/zed-fork-scan.sh {{ if old != '' { '--old ' + old + ' ' } else { '' } }}{{ if new != '' { '--new ' + new + ' ' } else { '' } }}{{ if mode != 'table' { '--' + mode } else { '' } }}

## 例：just zed-fork-scan                            # 当前 pin → origin/main
##     just zed-fork-commits                         # 逐条分诊
##     just zed-fork-scan bd747337 v1.22.0           # 显式区间
## 直接调脚本：tools/zed-fork-scan.sh --commits
zed-fork-scan old='' new='': (_fork-scan 'table' old new)
zed-fork-commits old='' new='': (_fork-scan 'commits' old new)
zed-fork-files old='' new='': (_fork-scan 'files' old new)
zed-fork-all old='' new='': (_fork-scan 'all' old new)

# cargo tree -e no-dev -i -p terminal_view    # 谁依赖 terminal_view（反向）
# cargo tree -e no-dev -p terminal_view       # terminal_view 依赖谁（正向）
# cargo tree --prefix none -p terminal_view   # 简洁版
