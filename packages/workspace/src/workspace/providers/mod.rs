// ├── providers/
// │   ├── mod.rs
// │   ├── terminal.rs            # TerminalProvider
// │   ├── debugger.rs            # DebuggerProvider
// │   └── active_call.rs         # AnyActiveCall / GlobalAnyActiveCall

use super::*;
pub mod debug;
pub mod terminal;
pub use terminal::TerminalProvider;