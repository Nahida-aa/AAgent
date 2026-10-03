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

# cargo tree -e no-dev -i -p terminal_view    # 谁依赖 terminal_view（反向）
# cargo tree -e no-dev -p terminal_view       # terminal_view 依赖谁（正向）
# cargo tree --prefix none -p terminal_view   # 简洁版
