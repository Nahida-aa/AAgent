use super::*;

#[derive(Debug, Default)]
pub enum PrepareRenameResponse {
    Success {
        range: Range<Anchor>,
        language_server_id: Option<LanguageServerId>,
    },
    OnlyUnpreparedRenameSupported,
    #[default]
    InvalidPosition,
}
