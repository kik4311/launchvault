use crate::models::*;
use std::fs;
use std::path::PathBuf;

pub const DATA_FILE: &str = "launchvault.json";

pub fn load() -> AppData {
    let path: PathBuf = app_data_dir().join(DATA_FILE);
    let Ok(text) = fs::read_to_string(&path) else {
        return AppData::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save(data: &AppData) {
    let dir = app_data_dir();
    let _ = fs::create_dir_all(&dir);
    let path = dir.join(DATA_FILE);
    if let Ok(text) = serde_json::to_string_pretty(data) {
        let _ = fs::write(path, text);
    }
}

pub fn ensure_dirs() {
    let _ = fs::create_dir_all(app_data_dir());
    let _ = fs::create_dir_all(covers_dir());
}