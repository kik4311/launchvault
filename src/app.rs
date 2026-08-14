use crate::models::*;
use crate::steam::*;
use crate::storage;
use eframe::egui;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Library,
    Steam,
    Settings,
}

struct AddGameDraft {
    name: String,
    path: String,
    args: String,
    cover: String,
    notes: String,
}

struct Session {
    game_id: String,
    started: u64,
    child: Option<std::process::Child>,
    kill_names: Vec<String>,
}

pub struct LaunchVaultApp {
    data: AppData,
    tab: Tab,
    search: String,
    selected: Option<String>,
    add_dialog: Option<AddGameDraft>,
    session: Option<Session>,
    confirm_kill: Option<String>,
    steam_games: Vec<SteamGame>,
    steam_error: Option<String>,
    steam_logos_queued: HashSet<u64>,
    last_save: f64,
    dark_mode: bool,
}

impl LaunchVaultApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        storage::ensure_dirs();
        let mut data = storage::load();

        let mut steam_games = Vec::new();
        let mut steam_error = None;
        let root = data
            .steam_path
            .as_deref()
            .map(PathBuf::from)
            .or_else(find_steam_root);
        match root {
            Some(p) => {
                data.steam_path = Some(p.to_string_lossy().to_string());
                steam_games = installed_games(&p);
            }
            None => steam_error = Some("Steam не найден — укажи путь в настройках".to_string()),
        }

        Self {
            data,
            tab: Tab::Library,
            search: String::new(),
            selected: None,
            add_dialog: None,
            session: None,
            confirm_kill: None,
            steam_games,
            steam_error,
            steam_logos_queued: HashSet::new(),
            last_save: 0.0,
            dark_mode: true,
        }
    }

    fn queue_steam_logos(&mut self, ctx: &egui::Context) {
        let ctx = ctx.clone();
        let appids: Vec<u64> = self
            .steam_games
            .iter()
            .map(|g| g.appid)
            .filter(|id| !self.steam_logos_queued.contains(id) && !logo_local_path(*id).exists())
            .collect();
        for id in appids {
            self.steam_logos_queued.insert(id);
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let _ = download_logo(id);
                ctx.request_repaint();
            });
        }
    }

    fn save(&mut self) {
        if self.session.is_some() {
            self.stop_session();
        }
        storage::save(&self.data);
    }

    fn stop_session(&mut self) {
        if let Some(s) = self.session.take() {
            let now = now_secs();
            let dur = now.saturating_sub(s.started);
            if let Some(mut child) = s.child {
                let _ = child.kill();
                let _ = child.wait();
            }
            kill_process(&s.kill_names);
            if let Some(g) = self.data.games.iter_mut().find(|g| g.id == s.game_id) {
                g.playtime_sec += dur;
                g.last_played = Some(s.started);
            }
        }
    }

    fn add_steam_library_game(&mut self, sg: &SteamGame) {
        let id = format!("steam-{}", sg.appid);
        if self.data.games.iter().any(|g| g.id == id) {
            return;
        }
        let cover = download_cover(sg.appid, &sg.name);
        self.data.games.push(Game {
            id,
            name: sg.name.clone(),
            source: Source::Steam,
            steam_id: Some(sg.appid),
            path: None,
            args: None,
            cover: cover.map(|p| p.to_string_lossy().to_string()),
            notes: None,
            playtime_sec: 0,
            last_played: None,
            added_at: now_secs(),
        });
        self.data
            .games
            .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    }

    fn launch_game(&mut self, id: &str) {
        let Some(game) = self.data.games.iter().find(|g| g.id == id).cloned() else {
            return;
        };
        let mut child = None;
        let mut kill_names = Vec::new();
        match game.source {
            Source::Steam => {
                if let Some(appid) = game.steam_id {
                    launch_steam_game(appid);
                    if let Some(root) = self.data.steam_path.clone().map(PathBuf::from) {
                        if root.join("steamapps").is_dir() {
                            kill_names = steam_game_process_names(&root, appid);
                        }
                    }
                }
            }
            Source::Local => {
                if let Some(p) = &game.path {
                    child = launch_local(p, &game.args);
                }
            }
        }
        self.session = Some(Session {
            game_id: id.to_string(),
            started: now_secs(),
            child,
            kill_names,
        });
    }

    fn total_playtime(&self) -> String {
        let total: u64 = self.data.games.iter().map(|g| g.playtime_sec).sum();
        let h = total / 3600;
        let m = (total % 3600) / 60;
        if h > 0 {
            format!("{h} ч {m} мин")
        } else {
            format!("{m} мин")
        }
    }

    fn placeholder_cover(ui: &mut egui::Ui, name: &str, size: egui::Vec2) {
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 8.0, egui::Color32::from_gray(28));
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            name.chars().next().unwrap_or('?').to_uppercase(),
            egui::FontId::proportional(44.0),
            egui::Color32::from_gray(110),
        );
    }

    fn show_side_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("side")
            .resizable(false)
            .default_size(220.0)
            .show(ui, |ui| {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Image::new(egui::include_image!("launchvault_icon.png"))
                            .max_size(egui::vec2(64.0, 64.0)),
                    );
                    ui.vertical(|ui| {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("LaunchVault").size(22.0).strong());
                        ui.colored_label(
                            egui::Color32::from_rgb(100, 180, 255),
                            egui::RichText::new("персональный игровой центр").small().weak(),
                        );
                    });
                });
                ui.add_space(16.0);

                let items = [
                    (
                        Tab::Library,
                        "Библиотека",
                        self.data.games.len().to_string(),
                    ),
                    (Tab::Steam, "Steam", self.steam_games.len().to_string()),
                    (Tab::Settings, "Настройки", String::new()),
                ];
                for (tab, label, count) in items {
                    let selected = self.tab == tab;
                    let text = if count.is_empty() {
                        label.to_string()
                    } else {
                        format!("{label}  ({count})")
                    };
                    if ui
                        .selectable_label(selected, egui::RichText::new(text).size(15.0))
                        .clicked()
                    {
                        self.tab = tab;
                        self.selected = None;
                    }
                    ui.add_space(2.0);
                }
                ui.add_space(20.0);
                ui.label(egui::RichText::new("Сессия").strong());
                match &self.session {
                    Some(s) => {
                        if let Some(g) = self.data.games.iter().find(|g| g.id == s.game_id) {
                            ui.label(format!("Играет: {}", g.name));
                            let secs = now_secs().saturating_sub(s.started);
                            ui.label(
                                egui::RichText::new(format!("{:02}:{:02}", secs / 60, secs % 60))
                                    .strong()
                                    .color(egui::Color32::from_rgb(120, 220, 120)),
                            );
                            ui.add_space(4.0);
                            let gid = s.game_id.clone();
                            if ui.button("Остановить сессию").clicked() {
                                self.confirm_kill = Some(gid);
                            }
                        }
                    }
                    None => {
                        ui.label("(ничего)");
                    }
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "Игр: {}\nОбщее время: {}",
                            self.data.games.len(),
                            self.total_playtime()
                        ))
                        .weak()
                        .small(),
                    );
                });
            });
    }

    fn show_library(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Библиотека");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("+ Игра").clicked() {
                    self.add_dialog = Some(AddGameDraft {
                        name: String::new(),
                        path: String::new(),
                        args: String::new(),
                        cover: String::new(),
                        notes: String::new(),
                    });
                }
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Поиск:");
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("название игры"));
            if ui.button("Сброс").clicked() {
                self.search.clear();
            }
        });
        ui.separator();

        let query = self.search.to_lowercase();
        let filtered: Vec<Game> = self
            .data
            .games
            .iter()
            .filter(|g| query.is_empty() || g.name.to_lowercase().contains(&query))
            .cloned()
            .collect();

        if filtered.is_empty() {
            ui.add_space(20.0);
            ui.centered_and_justified(|ui| {
                ui.label("Пусто. Добавь первую игру кнопкой «+ Игра».");
            });
            return;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("library_grid")
                    .spacing([14.0, 14.0])
                    .min_col_width(250.0)
                    .show(ui, |ui| {
                        for game in &filtered {
                            self.game_card(ui, game);
                            ui.end_row();
                        }
                    });
            });
    }

    fn game_card(&mut self, ui: &mut egui::Ui, game: &Game) {
        let selected = self.selected.as_deref() == Some(&game.id);
        let frame = egui::Frame::NONE
            .fill(if selected {
                egui::Color32::from_rgb(40, 45, 70)
            } else {
                egui::Color32::from_gray(24)
            })
            .corner_radius(10.0)
            .inner_margin(egui::Margin::same(10));
        frame.show(ui, |ui| {
            ui.set_width(250.0);
            match game.cover_uri() {
                Some(uri) => {
                    ui.add(
                        egui::Image::from_uri(uri)
                            .maintain_aspect_ratio(true)
                            .max_size(egui::vec2(250.0, 100.0))
                            .corner_radius(6),
                    );
                }
                None => Self::placeholder_cover(ui, &game.name, egui::vec2(250.0, 100.0)),
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if let Some(appid) = game.steam_id {
                    let logo = logo_local_path(appid);
                    if logo.exists() {
                        ui.add(
                            egui::Image::from_uri(format!("file://{}", logo.display()))
                                .maintain_aspect_ratio(true)
                                .max_height(24.0),
                        );
                    }
                }
                ui.label(egui::RichText::new(&game.name).strong().size(14.0));
            });
            ui.label(egui::RichText::new(game.playtime_readable()).weak().small());
            ui.horizontal(|ui| {
                if ui.button("Запуск").clicked() {
                    self.launch_game(&game.id);
                }
                if ui.button("Подробнее").clicked() {
                    self.selected = Some(game.id.clone());
                }
                let tag = match game.source {
                    Source::Local => "локальная",
                    Source::Steam => "steam",
                };
                ui.label(egui::RichText::new(tag).weak().small());
            });
        });
    }

    fn show_game_details(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let Some(game) = self.data.games.iter().find(|g| g.id == id).cloned() else {
            return;
        };
        egui::Panel::right("details")
            .default_size(360.0)
            .show(ui, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Назад").clicked() {
                        self.selected = None;
                    }
                    if let Some(appid) = game.steam_id {
                        let logo = logo_local_path(appid);
                        if logo.exists() {
                            ui.add(
                                egui::Image::from_uri(format!("file://{}", logo.display()))
                                    .maintain_aspect_ratio(true)
                                    .max_height(32.0),
                            );
                        }
                    }
                    ui.heading(&game.name);
                });
                ui.separator();
                match &game.cover_uri() {
                    Some(uri) => {
                        ui.add(
                            egui::Image::from_uri(uri)
                                .maintain_aspect_ratio(true)
                                .max_size(egui::vec2(340.0, 140.0))
                                .corner_radius(8),
                        );
                    }
                    None => Self::placeholder_cover(ui, &game.name, egui::vec2(340.0, 140.0)),
                }
                ui.add_space(8.0);
                egui::Grid::new("details_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Источник:");
                        ui.label(match game.source {
                            Source::Local => "Локальная игра".to_string(),
                            Source::Steam => {
                                format!("Steam ({})", game.steam_id.unwrap_or(0))
                            }
                        });
                        ui.end_row();
                        ui.label("Время:");
                        ui.label(game.playtime_readable());
                        ui.end_row();
                        ui.label("Последний запуск:");
                        ui.label(game.last_played_readable(now_secs()));
                        ui.end_row();
                        ui.label("Добавлена:");
                        ui.label({
                            let days = now_secs().saturating_sub(game.added_at) / 86400;
                            if days < 1 {
                                "сегодня".to_string()
                            } else {
                                format!("{days} дн. назад")
                            }
                        });
                        ui.end_row();
                        if let Some(p) = &game.path {
                            ui.label("Путь:");
                            ui.label(p);
                            ui.end_row();
                        }
                        if let Some(notes) = &game.notes {
                            ui.label("Заметки:");
                            ui.label(notes);
                            ui.end_row();
                        }
                    });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("Запустить").clicked() {
                        self.launch_game(&game.id);
                    }
                    if ui.button("Стоп").clicked() {
                        self.confirm_kill = Some(game.id.clone());
                    }
                });
                ui.add_space(8.0);
                if ui.button("Удалить из библиотеки").clicked() {
                    self.data.games.retain(|g| g.id != game.id);
                    self.selected = None;
                    self.save();
                }
            });
    }

    fn show_steam(&mut self, ui: &mut egui::Ui) {
        ui.heading("Steam");
        ui.label(
            "Установленные в Steam игры можно добавить в LaunchVault: запуск через Steam, обложки и время — локально.",
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Путь к Steam:");
            let path = self.data.steam_path.get_or_insert_default();
            ui.add(egui::TextEdit::singleline(path).desired_width(360.0));
            if ui.button("Синхронизировать").clicked() {
                let root = self.data.steam_path.clone().map(PathBuf::from);
                self.steam_games = match root {
                    Some(p) if p.join("steamapps").is_dir() => installed_games(&p),
                    _ => {
                        self.steam_error =
                            Some("Путь не найден или не содержит steamapps".to_string());
                        Vec::new()
                    }
                };
            }
        });
        if let Some(err) = &self.steam_error {
            ui.colored_label(egui::Color32::from_rgb(230, 120, 120), err);
            ui.add_space(4.0);
        }
        ui.separator();

        let synced: HashSet<String> = self
            .data
            .games
            .iter()
            .filter(|g| g.source == Source::Steam)
            .map(|g| g.id.clone())
            .collect();
        let games = self.steam_games.clone();
        self.queue_steam_logos(ui.ctx());

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for sg in &games {
                    let id = format!("steam-{}", sg.appid);
                    let added = synced.contains(&id);
                    ui.horizontal(|ui| {
                        let logo = logo_local_path(sg.appid);
                        if logo.exists() {
                            ui.add(
                                egui::Image::from_uri(format!("file://{}", logo.display()))
                                    .maintain_aspect_ratio(true)
                                    .max_height(28.0),
                            );
                        } else {
                            Self::placeholder_cover(ui, &sg.name, egui::vec2(44.0, 28.0));
                        }
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&sg.name).strong());
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} · {}",
                                    size_readable(sg.size_on_disk),
                                    sg.library.display()
                                ))
                                .weak()
                                .small(),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if added {
                                if ui.button("Запустить").clicked() {
                                    let gid = format!("steam-{}", sg.appid);
                                    if self.data.games.iter().any(|g| g.id == gid) {
                                        self.launch_game(&gid);
                                    } else {
                                        launch_steam_game(sg.appid);
                                    }
                                }
                                ui.label(egui::RichText::new("в библиотеке").weak().small());
                            } else if ui.button("+ Добавить").clicked() {
                                self.add_steam_library_game(sg);
                                self.save();
                            }
                        });
                    });
                    ui.separator();
                }
                if games.is_empty() && self.steam_error.is_none() {
                    ui.add_space(12.0);
                    ui.label("Установленных игр не найдено — нажми «Синхронизировать».");
                }
            });
    }

    fn show_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Настройки");
        ui.add_space(6.0);
        egui::CollapsingHeader::new("Внешний вид").show(ui, |ui| {
            if ui.checkbox(&mut self.dark_mode, "Тёмная тема").changed() {
                if self.dark_mode {
                    ui.ctx().set_visuals(egui::Visuals::dark());
                } else {
                    ui.ctx().set_visuals(egui::Visuals::light());
                }
            }
        });
        egui::CollapsingHeader::new("Steam").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Путь:");
                let path = self.data.steam_path.get_or_insert_default();
                ui.add(egui::TextEdit::singleline(path).desired_width(420.0));
                if ui.button("Автоопределить").clicked() {
                    if let Some(p) = find_steam_root() {
                        self.data.steam_path = Some(p.to_string_lossy().to_string());
                        self.steam_games = installed_games(&p);
                    } else {
                        self.steam_error = Some("Steam не найден".to_string());
                    }
                }
            });
        });
        egui::CollapsingHeader::new("Данные").show(ui, |ui| {
            ui.label(format!("Папка данных: {}", app_data_dir().display()));
            ui.label(format!("Обложки: {}", covers_dir().display()));
            if ui.button("Очистить кэш обложек").clicked() {
                let _ = std::fs::remove_dir_all(covers_dir());
                let _ = std::fs::create_dir_all(covers_dir());
                for g in &mut self.data.games {
                    if g.source == Source::Steam {
                        if let Some(steam_id) = g.steam_id {
                            let name = g.name.clone();
                            g.cover = download_cover(steam_id, &name)
                                .map(|p| p.to_string_lossy().to_string());
                        }
                    }
                }
                self.save();
            }
        });
        ui.separator();
        ui.label(
            egui::RichText::new(format!("LaunchVault {}", env!("CARGO_PKG_VERSION")))
                .weak()
                .small(),
        );
        ui.label(
            egui::RichText::new("© 2026 kik4311 · GPL-3.0")
                .weak()
                .small(),
        );
    }

    fn show_add_dialog(&mut self, ui: &mut egui::Ui) {
        let mut open = true;
        let mut submit = false;
        let mut cancel = false;
        egui::Window::new("Новая игра")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                let Some(draft) = self.add_dialog.as_mut() else {
                    return;
                };
                egui::Grid::new("add_grid")
                    .num_columns(2)
                    .spacing([10.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Название:");
                        ui.add(egui::TextEdit::singleline(&mut draft.name).desired_width(320.0));
                        ui.end_row();
                        ui.label("Путь к запуску:");
                        ui.add(
                            egui::TextEdit::singleline(&mut draft.path)
                                .hint_text("/path/to/game или команда")
                                .desired_width(320.0),
                        );
                        ui.end_row();
                        ui.label("Аргументы:");
                        ui.add(egui::TextEdit::singleline(&mut draft.args).desired_width(320.0));
                        ui.end_row();
                        ui.label("Обложка:");
                        ui.add(
                            egui::TextEdit::singleline(&mut draft.cover)
                                .hint_text("путь к файлу или http://")
                                .desired_width(320.0),
                        );
                        ui.end_row();
                        ui.label("Заметки:");
                        ui.add(
                            egui::TextEdit::multiline(&mut draft.notes)
                                .desired_width(320.0)
                                .desired_rows(2),
                        );
                        ui.end_row();
                    });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Добавить").clicked() {
                        submit = true;
                    }
                    if ui.button("Отмена").clicked() {
                        cancel = true;
                    }
                });
                if draft.name.trim().is_empty() {
                    ui.colored_label(
                        egui::Color32::from_rgb(230, 160, 80),
                        "Название обязательно",
                    );
                }
            });
        if submit {
            if let Some(draft) = self.add_dialog.take() {
                let name = draft.name.trim().to_string();
                if !name.is_empty() {
                    self.data.games.push(Game {
                        id: uuid::Uuid::new_v4().to_string(),
                        name,
                        source: Source::Local,
                        steam_id: None,
                        path: if draft.path.trim().is_empty() {
                            None
                        } else {
                            Some(draft.path.trim().to_string())
                        },
                        args: if draft.args.trim().is_empty() {
                            None
                        } else {
                            Some(draft.args.trim().to_string())
                        },
                        cover: if draft.cover.trim().is_empty() {
                            None
                        } else {
                            Some(draft.cover.trim().to_string())
                        },
                        notes: if draft.notes.trim().is_empty() {
                            None
                        } else {
                            Some(draft.notes.trim().to_string())
                        },
                        playtime_sec: 0,
                        last_played: None,
                        added_at: now_secs(),
                    });
                    self.data
                        .games
                        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                    self.save();
                }
            }
        }
        if !open || cancel {
            self.add_dialog = None;
        }
    }

    fn show_confirm_kill(&mut self, ui: &mut egui::Ui) {
        let Some(game_id) = self.confirm_kill.clone() else {
            return;
        };
        let game_name = self
            .data
            .games
            .iter()
            .find(|g| g.id == game_id)
            .map(|g| g.name.clone())
            .unwrap_or_else(|| "Игра".to_string());
        let mut open = true;
        let mut confirm = false;
        let mut cancel = false;
        egui::Window::new("Принудительное завершение")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.label(egui::RichText::new(format!("Завершить «{game_name}»?")).strong());
                ui.add_space(4.0);
                ui.label("Процесс игры будет принудительно закрыт.");
                ui.colored_label(
                    egui::Color32::from_rgb(230, 160, 80),
                    "Несохранённый прогресс может быть потерян!",
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(egui::RichText::new("Да, завершить").color(egui::Color32::from_rgb(230, 120, 120)))
                        .clicked()
                    {
                        confirm = true;
                    }
                    if ui.button("Отмена").clicked() {
                        cancel = true;
                    }
                });
            });
        if confirm {
            self.stop_session();
            self.confirm_kill = None;
        }
        if cancel || !open {
            self.confirm_kill = None;
        }
    }
}

impl eframe::App for LaunchVaultApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let time = ui.ctx().input(|i| i.time);
        if time - self.last_save > 30.0 {
            self.last_save = time;
            storage::save(&self.data);
        }

        self.show_side_panel(ui);

        if self.tab == Tab::Library && self.selected.is_some() {
            self.show_game_details(ui);
        }

        egui::Frame::central_panel(ui.style()).show(ui, |ui| match self.tab {
            Tab::Library => self.show_library(ui),
            Tab::Steam => self.show_steam(ui),
            Tab::Settings => self.show_settings(ui),
        });

        if self.add_dialog.is_some() {
            self.show_add_dialog(ui);
        }
        if self.confirm_kill.is_some() {
            self.show_confirm_kill(ui);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save();
    }
}