use crate::models::covers_dir;
use crate::vdf;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct SteamGame {
    pub appid: u64,
    pub name: String,
    pub installdir: String,
    pub size_on_disk: u64,
    pub library: PathBuf,
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn find_steam_root() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("STEAM_ROOT") {
        let p = PathBuf::from(p);
        if p.join("steamapps").is_dir() {
            return Some(p);
        }
    }
    let candidates = [
        dirs::home_dir().map(|h| h.join(".steam").join("steam")),
        dirs::home_dir().map(|h| h.join(".local").join("share").join("Steam")),
        dirs::home_dir().map(|h| h.join("snap").join("steam").join("common").join(".steam").join("steam")),
        std::env::var_os("ProgramFiles(x86)").map(PathBuf::from).map(|p| p.join("Steam")),
        std::env::var_os("ProgramFiles").map(PathBuf::from).map(|p| p.join("Steam")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|p| p.join("steamapps").is_dir())
}

fn vdf_file(steam_root: &Path) -> PathBuf {
    steam_root.join("steamapps").join("libraryfolders.vdf")
}

fn parse_library_paths(steam_root: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![steam_root.to_path_buf()];
    if let Ok(text) = fs::read_to_string(vdf_file(steam_root)) {
        if let Some(root) = vdf::parse(&text) {
            if let Some(map) = root.get("libraryfolders") {
                if let vdf::VdfValue::Map(entries) = map {
                    for (k, v) in entries {
                        if k == "0" || k.parse::<u32>().is_ok() {
                            if let Some(path) = v.get("path").and_then(|p| p.as_str()) {
                                let p = steam_root.join(path);
                                if p.join("steamapps").is_dir() {
                                    dirs.push(p);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    dirs
}

pub fn installed_games(steam_root: &Path) -> Vec<SteamGame> {
    let mut games = Vec::new();
    for lib in parse_library_paths(steam_root) {
        let apps = lib.join("steamapps");
        let Ok(entries) = fs::read_dir(&apps) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("appmanifest_") || !name.ends_with(".acf") {
                continue;
            }
            let Ok(text) = fs::read_to_string(entry.path()) else {
                continue;
            };
            let Some(parsed) = vdf::parse(&text) else {
                continue;
            };
            let state = parsed
                .get("AppState")
                .or(Some(&parsed))
                .expect("always");
            let appid: u64 = state
                .get("appid")
                .and_then(|v| v.as_str())
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            if appid == 0 {
                continue;
            }
            let name = state
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();
            let installdir = state
                .get("installdir")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let size_on_disk: u64 = state
                .get("SizeOnDisk")
                .and_then(|v| v.as_str())
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            games.push(SteamGame {
                appid,
                name,
                installdir,
                size_on_disk,
                library: lib.clone(),
            });
        }
    }
    games.sort_by_key(|g| g.name.to_lowercase());
    games
}

pub fn cover_url(appid: u64) -> String {
    format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/header.jpg")
}

pub fn logo_url(appid: u64) -> String {
    format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/logo.png")
}

pub fn cover_local_path(appid: u64, name: &str) -> PathBuf {
    let safe: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    covers_dir().join(format!("{appid}_{}.jpg", &safe[..safe.len().min(48)]))
}

pub fn download_cover(appid: u64, name: &str) -> Option<PathBuf> {
    let dest = cover_local_path(appid, name);
    if dest.exists() {
        return Some(dest);
    }
    download_file(&cover_url(appid), &dest)
}

pub fn logo_local_path(appid: u64) -> PathBuf {
    covers_dir().join(format!("{appid}_logo.png"))
}

pub fn download_logo(appid: u64) -> Option<PathBuf> {
    let dest = logo_local_path(appid);
    if dest.exists() {
        return Some(dest);
    }
    download_file(&logo_url(appid), &dest)
}

fn download_file(url: &str, dest: &std::path::Path) -> Option<PathBuf> {
    let Ok(mut resp) = ureq::get(url).call() else {
        return None;
    };
    let Ok(bytes) = resp.body_mut().with_config().limit(10_000_000).read_to_vec() else {
        return None;
    };
    if let Some(dir) = dest.parent() {
        let _ = fs::create_dir_all(dir);
    }
    fs::write(dest, bytes).ok()?;
    Some(dest.to_path_buf())
}

pub fn open_url(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = Command::new("cmd").args(["/C", "start", ""]).arg(url).spawn();
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open").arg(url).spawn();
    }
}

pub fn launch_steam_game(appid: u64) {
    open_url(&format!("steam://rungameid/{appid}"));
}

pub fn launch_local(path: &str, args: &Option<String>) -> Option<std::process::Child> {
    let mut cmd = Command::new(path);
    if let Some(a) = args {
        for arg in a.split_whitespace() {
            cmd.arg(arg);
        }
    }
    cmd.spawn().ok()
}

/// Имена исполняемых файлов установленной Steam-игры (для завершения процесса).
/// Возвращает уникальные имена процессов + installdir как fallback-цель.
pub fn steam_game_process_names(steam_root: &Path, appid: u64) -> Vec<String> {
    let mut names = Vec::new();
    for g in installed_games(steam_root) {
        if g.appid != appid {
            continue;
        }
        let exe_dir = g.library.join("steamapps").join("common").join(&g.installdir);
        names.push(g.installdir.clone());
        collect_executables(&exe_dir, 3, &mut names);
    }
    names.sort();
    names.dedup();
    names
}

fn collect_executables(dir: &Path, depth: u32, out: &mut Vec<String>) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if name == "bin" || name == "linux64" || name == "game" || name == "scripts" {
                collect_executables(&path, depth - 1, out);
            }
            continue;
        }
        let is_exe = path
            .extension()
            .map(|e| e == "exe" || e == "sh" || e == "bin" || e == "x86_64" || e == "run")
            .unwrap_or(false);
        let has_exec_bit = {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                path.metadata()
                    .map(|m| m.permissions().mode() & 0o111 != 0)
                    .unwrap_or(false)
            }
            #[cfg(not(unix))]
            {
                false
            }
        };
        if is_exe || has_exec_bit {
            out.push(name);
        }
    }
}

/// Завершить процесс по имени (Unix: pkill -f; Windows: taskkill).
pub fn kill_process(names: &[String]) {
    if names.is_empty() {
        return;
    }
    for name in names {
        #[cfg(target_os = "windows")]
        let _ = Command::new("taskkill").args(["/IM", name, "/F"]).spawn();
        #[cfg(not(target_os = "windows"))]
        let _ = Command::new("pkill").arg("-f").arg(name).spawn();
    }
}

pub fn size_readable(bytes: u64) -> String {
    const GB: u64 = 1024 * 1024 * 1024;
    const MB: u64 = 1024 * 1024;
    if bytes >= GB {
        format!("{:.1} ГБ", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0} МБ", bytes as f64 / MB as f64)
    } else {
        format!("{:.0} КБ", bytes as f64 / 1024.0)
    }
}