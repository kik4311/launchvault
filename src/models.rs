use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    Local,
    Steam,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub source: Source,
    pub steam_id: Option<u64>,
    pub path: Option<String>,
    pub args: Option<String>,
    pub cover: Option<String>,
    pub notes: Option<String>,
    pub playtime_sec: u64,
    pub last_played: Option<u64>,
    pub added_at: u64,
}

impl Game {
    pub fn cover_uri(&self) -> Option<String> {
        self.cover.as_ref().map(|c| {
            if c.starts_with("http") || c.starts_with("file:") {
                c.clone()
            } else {
                format!("file://{}", c)
            }
        })
    }

    pub fn playtime_readable(&self) -> String {
        let h = self.playtime_sec / 3600;
        let m = (self.playtime_sec % 3600) / 60;
        if h > 0 {
            format!("{} ч {} мин", h, m)
        } else {
            format!("{} мин", m)
        }
    }

    pub fn last_played_readable(&self, now: u64) -> String {
        match self.last_played {
            None => "никогда".to_string(),
            Some(ts) => {
                let days = (now - ts) / 86400;
                if days == 0 {
                    "сегодня".to_string()
                } else if days == 1 {
                    "вчера".to_string()
                } else {
                    format!("{} дн. назад", days)
                }
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppData {
    pub games: Vec<Game>,
    pub steam_path: Option<String>,
    pub steam_cached: bool,
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            games: Vec::new(),
            steam_path: None,
            steam_cached: false,
        }
    }
}

pub fn app_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("launchvault")
}

pub fn covers_dir() -> PathBuf {
    app_data_dir().join("covers")
}