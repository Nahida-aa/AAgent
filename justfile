run-app:
    cargo run -p aa-app

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
