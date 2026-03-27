use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ClipboardHistory {
    entries: Vec<ClipboardEntry>,
}

impl ClipboardHistory {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::storage_path()?;

        if path.exists() {
            let data = fs::read_to_string(&path)?;
            let history: ClipboardHistory = serde_json::from_str(&data)?;
            Ok(history)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::storage_path()?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let data = serde_json::to_string_pretty(self)?;
        fs::write(&path, data)?;
        Ok(())
    }

    pub fn add(&mut self, content: String, max_size: usize) {
        // Don't add duplicates of the most recent entry
        if let Some(last) = self.entries.first() {
            if last.content == content {
                return;
            }
        }

        let entry = ClipboardEntry {
            content,
            timestamp: Utc::now(),
        };

        // Insert at front (most recent first)
        self.entries.insert(0, entry);

        // Trim to max size
        self.entries.truncate(max_size);
    }

    pub fn entries(&self) -> &[ClipboardEntry] {
        &self.entries
    }

    pub fn get(&self, index: usize) -> Option<&ClipboardEntry> {
        self.entries.get(index)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    fn storage_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let data_dir = dirs::data_local_dir()
            .ok_or("Could not find local data directory")?;
        Ok(data_dir.join("clipboard-history").join("history.json"))
    }
}
