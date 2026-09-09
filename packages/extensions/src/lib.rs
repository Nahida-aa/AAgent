//! AAgent Extension 加载器和运行时。

pub mod host_router;
pub mod loader;
pub mod registry;
pub mod wasm;

pub use host_router::HostRouter;
pub use loader::{ExtensionLoader, Manifest};
pub use registry::ExtensionRegistry;
