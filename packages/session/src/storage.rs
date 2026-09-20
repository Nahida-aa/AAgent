use std::path::PathBuf;

use aa_core::llm::Message;
use serde::{Deserialize, Serialize};

/// A persisted session file on disk.
#[derive(Serialize, Deserialize)]
pub struct SessionFile {
    pub session_id: String,
    pub model: String,
    pub provider: String,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<Message>,
}

/// Default storage directory: `~/.local/share/aa/sessions/`
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(dir).join("aa").join("sessions")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("aa")
            .join("sessions")
    } else {
        PathBuf::from(".aa").join("sessions")
    }
}

fn session_path(session_id: &str) -> PathBuf { data_dir().join(format!("{session_id}.json")) }

/// Save (or update) a session file.
pub fn save(
    session_id: &str,
    messages: &[Message],
    model: &str,
    provider: &str,
) -> anyhow::Result<()> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;

    let now = chrono::Local::now().to_rfc3339();
    let path = session_path(session_id);

    let file = if path.exists() {
        let mut existing: SessionFile = serde_json::from_reader(std::fs::File::open(&path)?)?;
        existing.messages = messages.to_vec();
        existing.model = model.into();
        existing.provider = provider.into();
        existing.updated_at = now;
        existing
    } else {
        SessionFile {
            session_id: session_id.into(),
            model: model.into(),
            provider: provider.into(),
            created_at: now.clone(),
            updated_at: now,
            messages: messages.to_vec(),
        }
    };

    let f = std::fs::File::create(&path)?;
    serde_json::to_writer_pretty(f, &file)?;
    Ok(())
}

/// Load messages for a session.
pub fn load(session_id: &str) -> anyhow::Result<Vec<Message>> {
    let path = session_path(session_id);
    let file: SessionFile = serde_json::from_reader(std::fs::File::open(&path)?)?;
    Ok(file.messages)
}

/// Load the full session file.
pub fn load_file(session_id: &str) -> anyhow::Result<SessionFile> {
    let path = session_path(session_id);
    Ok(serde_json::from_reader(std::fs::File::open(&path)?)?)
}

/// List all saved sessions (sorted by most recent).
pub fn list() -> anyhow::Result<Vec<SessionFile>> {
    let dir = data_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut sessions = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.path().extension().map_or(true, |e| e != "json") {
            continue;
        }
        if let Ok(file) = std::fs::File::open(entry.path()) {
            if let Ok(session) = serde_json::from_reader::<_, SessionFile>(file) {
                sessions.push(session);
            }
        }
    }

    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(sessions)
}

/// Delete a session file.
pub fn delete(session_id: &str) -> anyhow::Result<()> {
    let path = session_path(session_id);
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}
