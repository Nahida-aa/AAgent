use std::sync::LazyLock;
use std::time::Duration;

pub const INITIAL_RECONNECTION_DELAY: Duration = Duration::from_millis(500);
pub const MAX_RECONNECTION_DELAY: Duration = Duration::from_secs(30);
pub const CONNECTION_TIMEOUT: Duration = Duration::from_secs(20);

static ZED_RPC_URL: LazyLock<Option<String>> = LazyLock::new(|| std::env::var("ZED_RPC_URL").ok());
