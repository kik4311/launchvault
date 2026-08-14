#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
mod gtk;

#[cfg(target_os = "linux")]
fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .init();
    gtk::run();
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!(
        "LaunchVault на Windows собирается как WPF-приложение (см. windows/LaunchVault)."
    );
    std::process::exit(1);
}
