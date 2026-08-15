#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod gtk;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .init();
    gtk::run();
}
