# 从 zed 搬包到 aacode：踩过的坑与做法

搬 `project` 时（1000+ 错误 → 0）沉淀的规则。参照仓库在 `~/repos/learn_ls/zed`。
**zed 源码是第一参照**，gpui-component / 自己的直觉都靠后。

**aacode crate edition: 2024** — 规则 #1 基于 Rust 2024 可见性变化。

---

## 1. Rust 2024 Edition 可见性规则（最重要）

**这一节是搬代码的基础。没搞懂之前别碰可见性。**

### 1.1 核心模型：模块是树形的

```
lib.rs (crate 根)
  ├── pub mod element;        ← pub: crate 内外都可见
  ├── mod panel;              ← Rust 2024 = pub(crate), 整个 crate 可见
  ├── mod persistence;        ← 同上
  └── pub mod view;
        ├── mod mode;         ← 私有, 只有 view/ 内部可见
        └── mod hover;        ← 同上
```

### 1.2 Rust 2024 独有的变化

| 位置 | 写法 | 实际含义 |
|------|------|---------|
| **lib.rs (根)** | `mod xxx;` | 整个 crate 可见 (= `pub(crate) mod`) |
| **lib.rs (根)** | `pub(crate) mod xxx;` | 和上面**完全一样**, 多余, rustc 会警告 |
| **lib.rs (根)** | `pub mod xxx;` | crate 内外都可见 |
| **子模块 mod.rs** | `mod xxx;` | 父模块 + 子模块可见 |
| **子模块 mod.rs** | `pub(crate) mod xxx;` | 整个 crate 可见 |
| **子模块 mod.rs** | `pub mod xxx;` | crate 内外都可见 |

**关键点: lib.rs 里的私有 mod 在 Rust 2024 下自动 pub(crate)。pub(crate) 前缀多余。**

### 1.3 兄弟模块互访

| 路径 | 含义 | 在哪能用 |
|------|------|---------|
| `super::xxx` | 从父节点出发 | **只能在子模块里** (lib.rs 没有父节点, 会 E0433) |
| `crate::xxx` | 从 crate 根出发 | 任何地方 |
| 隐式 `xxx::yyy` | 同上, 但更短 | 任何地方 |

兄弟互访私有子模块时:
- `super::bro::field` ✅ (子模块里, 从父节点出发找兄弟)
- `crate::bro::field` ✅ (任何地方, 但 bro 必须是 pub 或 Rust 2024 私有 = pub(crate))

### 1.4 转发块 (pub use) 的真正用途

旧版说"必须 pub use, 不能 pub(crate) use" — 这在**子模块 mod.rs** 里是对的,
但在 **lib.rs** 里是多余的 (Rust 2024 下私有 mod 已经全 crate 可见)。

转发块的唯一作用: **让外部 crate 能访问**。

如果某个类型只给 crate 内部用:
- 不需要转发块
- 调用点自己写 `use crate::xxx::yyy;`

如果某个类型要给外部 crate 用:
- 在**子模块 mod.rs** 里写 `pub use xxx::yyy;` (不是 pub(crate) use)
- 在 **lib.rs** 里只需要 `pub mod 子模块;`, 不需要额外 pub use

### 1.5 不要做的事

- ❌ **不要在 lib.rs 里加 `pub(crate) use` 共享导出块** — Rust 2024 私有 mod 已经全 crate 可见
- ❌ **不要搞 prelude.rs** — 每个文件独立加自己需要的 use
- ❌ **不要盲目改私有 mod 为 pub(crate)** — 先确认是不是兄弟模块真的需要跨 crate 访问
- ❌ **不要在 lib.rs 里写 `super::xxx`** — crate 根没有父节点, 会 E0433
- ❌ **不要把可见性修复等同于"加共享导出块"** — 90% 的情况调用点自己加 use 就行

### 1.6 验证

完整可见性验证实验见 `docs/visibility_lab/`:
```bash
cd docs/visibility_lab && cargo check
```
所有 ✅ 注释的行能编译, ❌ 取消注释会看到对应的 E0603/E0432/E0433。

---

## 2. 单文件 crate 拆多模块: 转发块什么时候需要

> 这一节描述的是 **zed 原版 (单文件 project.rs) 拆成 aacode 多模块** 的场景。
> 不是通用可见性修复模板。

zed 的 `project.rs` (7000+ 行) 是 crate 根, 所有定义天然对整个 crate 可见。
我们拆成 `project/mod.rs` + `project/types.rs` + `project/buffers.rs` 后:

- **如果类型还在 lib.rs** → Rust 2024 下私有 mod 已经全 crate 可见, 不需要转发
- **如果类型搬到了子模块** (比如 `project/types.rs` 里定义) → crate 内部用 `use crate::project::types::X`,
  外部 crate 要用才在 `project/mod.rs` 加 `pub use types::X;`

转发块的三条经验 (来自 project crate, 仍然有效):

- **不要转发 `proto`**。子模块自己写 `use ::rpc::proto;`。
  否则 `use super::*` 把 `proto` 带进子模块, 文件里的 `::rpc::proto::X`
  会被解析成 `proto::proto::X`。
- 同理不要转发与子模块同名的东西。

判定方法: 子模块里出现「找不到 X」时, 先去 zed 确认 X 是不是 crate 根里的裸名。
但**先别急着加转发块** — Rust 2024 下大概率是调用点自己加 `use crate::xxx::X` 就够了。

## 3. 模块名与外部 crate 同名时用绝对路径

