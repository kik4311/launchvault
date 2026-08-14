use adw::prelude::*;
use glib::ControlFlow;
use gtk::{Align, Orientation};
use launchvault_core::*;
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

const CSS: &str = r#"
.card { border-radius: 12px; background: @card_bg_color; }
.cover-ph { background: alpha(@theme_bg_color, 0.65); border: 1px solid alpha(@theme_fg_color, 0.08); border-radius: 8px; }
.cover-ph-letter { font-size: 26pt; font-weight: 800; color: alpha(@theme_fg_color, 0.4); }
.game-title { font-weight: 600; }
.game-meta { margin-top: 2px; }
.session-timer { font-weight: 700; color: @success_color; }
.weak-text { opacity: 0.7; }
"#;

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Library,
    Steam,
    Settings,
}

struct Session {
    game_id: String,
    started: u64,
    child: Option<std::process::Child>,
    kill_names: Vec<String>,
}

struct AppState {
    data: AppData,
    steam_games: Vec<SteamGame>,
    steam_error: Option<String>,
    session: Option<Session>,
    page: Page,
    search: String,
    cover_queued: HashSet<String>,
    logo_queued: HashSet<u64>,
    last_save: u64,
    window: adw::ApplicationWindow,
    toast: adw::ToastOverlay,
    nav: adw::NavigationView,
    root_page: adw::NavigationPage,
    stack: gtk::Stack,
    library_flow: gtk::Box,
    library_empty: gtk::Box,
    library_btn: gtk::ToggleButton,
    steam_btn: gtk::ToggleButton,
    settings_btn: gtk::ToggleButton,
    session_box: gtk::Box,
    session_idle: gtk::Label,
    session_game: gtk::Label,
    session_timer: gtk::Label,
    stats_label: gtk::Label,
    steam_list: gtk::ListBox,
    steam_hint: gtk::Label,
    search_entry: gtk::SearchEntry,
    refresh_tx: std::sync::mpsc::Sender<RefreshMsg>,
    back_btn: gtk::Button,
}

impl AppState {
    fn save(&mut self) {
        storage::save(&self.data);
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

    fn update_session_ui(&self) {
        let playing = self.session.is_some();
        self.session_box.set_visible(playing);
        self.session_idle.set_visible(!playing);
        if let Some(s) = &self.session {
            let name = self
                .data
                .games
                .iter()
                .find(|g| g.id == s.game_id)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| "?".to_string());
            self.session_game.set_text(&format!("Играет: {name}"));
            let secs = now_secs().saturating_sub(s.started);
            self.session_timer
                .set_text(&format!("{:02}:{:02}", secs / 60, secs % 60));
        }
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
                    let is_exe = p.trim().to_lowercase().ends_with(".exe");
                    if is_exe && self.data.umu.enabled {
                        child = launch_local_umu(p, &game.args, &self.data.umu, &game.id);
                    } else {
                        child = launch_local(p, &game.args);
                    }
                }
            }
        }
        self.session = Some(Session {
            game_id: id.to_string(),
            started: now_secs(),
            child,
            kill_names,
        });
        self.update_session_ui();
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
            self.save();
        }
        self.update_session_ui();
    }

    fn switch_page(&mut self, page: Page) {
        self.page = page;
        let (name, title) = match page {
            Page::Library => ("library", "Библиотека"),
            Page::Steam => ("steam", "Steam"),
            Page::Settings => ("settings", "Настройки"),
        };
        self.nav.pop_to_page(&self.root_page);
        self.stack.set_visible_child_name(name);
        self.root_page.set_title(title);
    }
}

fn custom_cover_dest(id: &str) -> PathBuf {
    let safe: String = id
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect();
    covers_dir().join(format!("{safe}_cover.png"))
}

fn local_cover_path(game: &Game) -> Option<PathBuf> {
    let steam_fallback = if game.source == Source::Steam {
        game.steam_id.map(|id| cover_local_path(id, &game.name))
    } else {
        None
    };
    match &game.cover {
        Some(c) if c.starts_with("file://") => Some(PathBuf::from(&c[7..])),
        Some(c) if c.starts_with("http") => {
            let d = custom_cover_dest(&game.id);
            if d.exists() {
                Some(d)
            } else {
                None
            }
        }
        Some(c) => {
            let p = PathBuf::from(c);
            if p.exists() {
                Some(p)
            } else {
                steam_fallback
            }
        }
        None => steam_fallback,
    }
}

fn placeholder_cover(name: &str, w: i32, h: i32) -> gtk::Box {
    let b = gtk::Box::new(Orientation::Vertical, 0);
    b.set_width_request(w);
    b.set_height_request(h);
    b.add_css_class("cover-ph");
    let letter = name.chars().next().unwrap_or('?').to_uppercase().to_string();
    let l = gtk::Label::new(Some(&letter));
    l.add_css_class("cover-ph-letter");
    l.set_halign(Align::Center);
    l.set_valign(Align::Center);
    b.append(&l);
    b
}

fn cover_picture(path: &Path, w: i32, h: i32) -> gtk::Picture {
    use gtk::gdk::gdk_pixbuf::InterpType;
    #[allow(deprecated)]
    let p = gtk::Picture::new();
    p.set_content_fit(gtk::ContentFit::Cover);
    p.set_can_shrink(true);
    p.set_halign(Align::Fill);
    p.set_valign(Align::Fill);
    p.set_size_request(w, h);
    if let Ok(pb) = gtk::gdk::gdk_pixbuf::Pixbuf::from_file(path) {
        if let Some(scaled) = pb.scale_simple(w, h, InterpType::Bilinear) {
            #[allow(deprecated)]
            let tex = gtk::gdk::Texture::for_pixbuf(&scaled);
            p.set_paintable(Some(&tex));
        }
    }
    p
}

fn refresh_library(state: &Rc<RefCell<AppState>>) {
    {
        let mut s = state.borrow_mut();
        for g in &mut s.data.games {
            if let Some(c) = g.cover.clone() {
                if c.starts_with("http") {
                    let d = custom_cover_dest(&g.id);
                    if d.exists() {
                        g.cover = Some(d.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }
    let games: Vec<Game> = {
        let s = state.borrow();
        let q = s.search.trim().to_lowercase();
        s.data
            .games
            .iter()
            .filter(|g| q.is_empty() || g.name.to_lowercase().contains(&q))
            .cloned()
            .collect()
    };
    let flow = state.borrow().library_flow.clone();
    while let Some(c) = flow.first_child() {
        flow.remove(&c);
    }
    const PER_LINE: usize = 4;
    let mut row: Option<gtk::Box> = None;
    for (i, game) in games.iter().enumerate() {
        if i % PER_LINE == 0 {
            let r = gtk::Box::new(Orientation::Horizontal, 12);
            r.set_halign(Align::Start);
            flow.append(&r);
            row = Some(r);
        }
        if let Some(r) = &row {
            let card = build_game_card(state, game);
            r.append(&card);
        }
    }
    let (empty, count_label, stats) = {
        let s = state.borrow();
        (
            s.library_empty.clone(),
            s.library_btn.clone(),
            s.stats_label.clone(),
        )
    };
    empty.set_visible(games.is_empty());
    let total = state.borrow().data.games.len();
    count_label.set_label(&format!("Библиотека ({total})"));
    stats.set_label(&format!(
        "Игр: {total}\nОбщее время: {}",
        state.borrow().total_playtime()
    ));
    queue_missing_covers(state);
}

fn build_game_card(state: &Rc<RefCell<AppState>>, game: &Game) -> gtk::Box {
    let card = gtk::Box::new(Orientation::Vertical, 6);
    card.add_css_class("card");
    card.set_size_request(240, -1);

    if let Some(p) = local_cover_path(game) {
        if p.exists() {
            card.append(&cover_picture(&p, 240, 112));
        } else {
            card.append(&placeholder_cover(&game.name, 240, 112));
        }
    } else {
        card.append(&placeholder_cover(&game.name, 240, 112));
    }

    let name = gtk::Label::new(Some(&game.name));
    name.set_wrap(true);
    name.set_lines(2);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    name.set_xalign(0.0);
    name.add_css_class("game-title");
    card.append(&name);

    let meta = gtk::Box::new(Orientation::Horizontal, 6);
    meta.add_css_class("game-meta");

    let pt = gtk::Label::new(Some(&game.playtime_readable()));
    pt.set_xalign(0.0);
    pt.set_hexpand(true);
    pt.add_css_class("weak-text");
    meta.append(&pt);

    let tag = match game.source {
        Source::Local => "локальная",
        Source::Steam => "steam",
    };
    let t = gtk::Label::new(Some(tag));
    t.set_xalign(1.0);
    t.add_css_class("weak-text");
    meta.append(&t);

    card.append(&meta);

    let buttons = gtk::Box::new(Orientation::Horizontal, 6);

    let launch = gtk::Button::with_label("Запуск");
    launch.add_css_class("suggested-action");
    launch.set_hexpand(true);
    let state_c = state.clone();
    let gid = game.id.clone();
    launch.connect_clicked(move |_| {
        state_c.borrow_mut().launch_game(&gid);
    });
    buttons.append(&launch);

    let details = gtk::Button::with_label("Подробнее");
    let state_c = state.clone();
    let gid = game.id.clone();
    details.connect_clicked(move |_| open_details(&state_c, &gid));
    buttons.append(&details);

    card.append(&buttons);
    card
}

fn open_details(state: &Rc<RefCell<AppState>>, gid: &str) {
    let game = {
        let s = state.borrow();
        s.data.games.iter().find(|g| g.id == gid).cloned()
    };
    if let Some(game) = game {
        let page = build_details_page(state, &game);
        state.borrow().nav.push(&page);
    }
}

fn build_details_page(state: &Rc<RefCell<AppState>>, game: &Game) -> adw::NavigationPage {
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);

    let b = gtk::Box::new(Orientation::Vertical, 10);
    b.set_margin_top(16);
    b.set_margin_bottom(16);
    b.set_margin_start(20);
    b.set_margin_end(20);
    b.set_width_request(560);
    b.set_halign(Align::Center);

    if let Some(p) = local_cover_path(game) {
        if p.exists() {
            b.append(&cover_picture(&p, 460, 215));
        } else {
            b.append(&placeholder_cover(&game.name, 460, 215));
        }
    } else {
        b.append(&placeholder_cover(&game.name, 460, 215));
    }

    let title = gtk::Label::new(Some(&game.name));
    title.add_css_class("title-1");
    title.set_xalign(0.0);
    b.append(&title);

    let info: Vec<(String, String)> = vec![
        (
            "Источник".to_string(),
            match game.source {
                Source::Local => "Локальная игра".to_string(),
                Source::Steam => format!("Steam ({})", game.steam_id.unwrap_or(0)),
            },
        ),
        ("Время".to_string(), game.playtime_readable()),
        (
            "Последний запуск".to_string(),
            game.last_played_readable(now_secs()),
        ),
        (
            "Добавлена".to_string(),
            {
                let days = now_secs().saturating_sub(game.added_at) / 86400;
                if days < 1 {
                    "сегодня".to_string()
                } else {
                    format!("{days} дн. назад")
                }
            },
        ),
    ];
    for (k, v) in info {
        let row = gtk::Box::new(Orientation::Horizontal, 12);
        let kk = gtk::Label::new(Some(&k));
        kk.add_css_class("weak-text");
        kk.set_width_request(150);
        let vv = gtk::Label::new(Some(&v));
        vv.set_xalign(0.0);
        vv.set_wrap(true);
        row.append(&kk);
        row.append(&vv);
        b.append(&row);
    }
    if let Some(p) = &game.path {
        let row = gtk::Box::new(Orientation::Horizontal, 12);
        let kk = gtk::Label::new(Some("Путь"));
        kk.add_css_class("weak-text");
        kk.set_width_request(150);
        let vv = gtk::Label::new(Some(p));
        vv.set_xalign(0.0);
        vv.set_wrap(true);
        row.append(&kk);
        row.append(&vv);
        b.append(&row);
    }
    if let Some(n) = &game.notes {
        let row = gtk::Box::new(Orientation::Horizontal, 12);
        let kk = gtk::Label::new(Some("Заметки"));
        kk.add_css_class("weak-text");
        kk.set_width_request(150);
        let vv = gtk::Label::new(Some(n));
        vv.set_xalign(0.0);
        vv.set_wrap(true);
        row.append(&kk);
        row.append(&vv);
        b.append(&row);
    }

    let buttons = gtk::Box::new(Orientation::Horizontal, 6);
    let launch = gtk::Button::with_label("Запустить");
    launch.add_css_class("suggested-action");
    let state_c = state.clone();
    let gid = game.id.clone();
    launch.connect_clicked(move |_| state_c.borrow_mut().launch_game(&gid));
    buttons.append(&launch);

    let stop = gtk::Button::with_label("Стоп");
    let state_c = state.clone();
    let gid = game.id.clone();
    stop.connect_clicked(move |_| confirm_stop(&state_c, &gid));
    buttons.append(&stop);

    let del = gtk::Button::with_label("Удалить из библиотеки");
    del.add_css_class("destructive-action");
    let state_c = state.clone();
    let gid = game.id.clone();
    del.connect_clicked(move |_| {
        let nav = state_c.borrow().nav.clone();
        let mut s = state_c.borrow_mut();
        s.data.games.retain(|g| g.id != gid);
        s.save();
        drop(s);
        refresh_library(&state_c);
        nav.pop();
    });
    buttons.append(&del);
    b.append(&buttons);

    scrolled.set_child(Some(&b));
    adw::NavigationPage::new(&scrolled, &game.name)
}

fn refresh_steam(state: &Rc<RefCell<AppState>>) {
    let synced: HashSet<String> = {
        let s = state.borrow();
        s.data
            .games
            .iter()
            .filter(|g| g.source == Source::Steam)
            .map(|g| g.id.clone())
            .collect()
    };
    let games: Vec<SteamGame> = { state.borrow().steam_games.clone() };
    let list = state.borrow().steam_list.clone();
    while let Some(r) = list.row_at_index(0) {
        list.remove(&r);
    }
    for sg in &games {
        let row = build_steam_row(state, sg, synced.contains(&format!("steam-{}", sg.appid)));
        state.borrow().steam_list.append(&row);
    }
    let (hint, btn) = {
        let s = state.borrow();
        (s.steam_hint.clone(), s.steam_btn.clone())
    };
    let err = state.borrow().steam_error.clone();
    let n = state.borrow().steam_games.len();
    btn.set_label(&format!("Steam ({n})"));
    match err {
        Some(e) => hint.set_text(&e),
        None if games.is_empty() => {
            hint.set_text("Установленных игр не найдено — нажми «Синхронизировать».")
        }
        None => hint.set_text(&format!("Найдено игр: {n}")),
    }
    queue_missing_logos(state);
}

fn build_steam_row(
    state: &Rc<RefCell<AppState>>,
    sg: &SteamGame,
    added: bool,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    let h = gtk::Box::new(Orientation::Horizontal, 12);
    h.set_margin_top(6);
    h.set_margin_bottom(6);
    h.set_margin_start(12);
    h.set_margin_end(12);

    let logo = logo_local_path(sg.appid);
    if logo.exists() {
        let pic = gtk::Picture::new();
        pic.set_content_fit(gtk::ContentFit::Contain);
        pic.set_can_shrink(true);
        pic.set_width_request(80);
        pic.set_height_request(28);
        let _ = pic.set_filename(Some(&logo));
        h.append(&pic);
    } else {
        h.append(&placeholder_cover(&sg.name, 80, 28));
    }

    let v = gtk::Box::new(Orientation::Vertical, 2);
    let name = gtk::Label::new(Some(&sg.name));
    name.set_xalign(0.0);
    name.add_css_class("game-title");
    let sub = gtk::Label::new(Some(&format!(
        "{} · {}",
        size_readable(sg.size_on_disk),
        sg.library.display()
    )));
    sub.set_xalign(0.0);
    sub.add_css_class("weak-text");
    v.append(&name);
    v.append(&sub);
    h.append(&v);

    let right = gtk::Box::new(Orientation::Horizontal, 6);
    right.set_hexpand(true);
    right.set_halign(Align::End);

    if added {
        let btn = gtk::Button::with_label("Запустить");
        let state_c = state.clone();
        let gid = format!("steam-{}", sg.appid);
        btn.connect_clicked(move |_| {
            let is_in = state_c.borrow().data.games.iter().any(|g| g.id == gid);
            if is_in {
                state_c.borrow_mut().launch_game(&gid);
            } else {
                launch_steam_game(gid[6..].parse().unwrap_or(0));
            }
        });
        right.append(&btn);
        let inlib = gtk::Label::new(Some("в библиотеке"));
        inlib.add_css_class("weak-text");
        right.append(&inlib);
    } else {
        let btn = gtk::Button::with_label("+ Добавить");
        btn.add_css_class("suggested-action");
        let state_c = state.clone();
        let appid = sg.appid;
        let name = sg.name.clone();
        btn.connect_clicked(move |_| {
            add_steam_game(&state_c, appid, &name);
        });
        right.append(&btn);
    }
    h.append(&right);

    row.set_child(Some(&h));
    row
}

fn add_steam_game(state: &Rc<RefCell<AppState>>, appid: u64, name: &str) {
    let id = format!("steam-{appid}");
    if state.borrow().data.games.iter().any(|g| g.id == id) {
        return;
    }
    let cover = download_cover(appid, name).map(|p| p.to_string_lossy().into_owned());
    let mut s = state.borrow_mut();
    s.data.games.push(Game {
        id,
        name: name.to_string(),
        source: Source::Steam,
        steam_id: Some(appid),
        path: None,
        args: None,
        cover,
        notes: None,
        playtime_sec: 0,
        last_played: None,
        added_at: now_secs(),
    });
    s.data
        .games
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    s.save();
    drop(s);
    refresh_library(state);
    let toast = adw::Toast::new(&format!("{name} добавлена в библиотеку"));
    state.borrow().toast.add_toast(toast);
}

fn queue_missing_covers(state: &Rc<RefCell<AppState>>) {
    let jobs: Vec<(String, u64, String, String)> = {
        let s = state.borrow();
        let mut jobs = Vec::new();
        for g in &s.data.games {
            if s.cover_queued.contains(&g.id) {
                continue;
            }
            let job = match &g.cover {
                Some(c) if c.starts_with("http") => {
                    let dest = custom_cover_dest(&g.id);
                    if dest.exists() {
                        None
                    } else {
                        Some((g.id.clone(), 0, c.clone(), String::new()))
                    }
                }
                _ => {
                    if g.source == Source::Steam {
                        match g.steam_id {
                            Some(appid) => {
                                let p = cover_local_path(appid, &g.name);
                                if p.exists() {
                                    None
                                } else {
                                    Some((
                                        g.id.clone(),
                                        appid,
                                        String::new(),
                                        g.name.clone(),
                                    ))
                                }
                            }
                            None => None,
                        }
                    } else {
                        None
                    }
                }
            };
            if let Some(j) = job {
                jobs.push(j);
            }
        }
        jobs
    };
    for (id, appid, url, name) in jobs {
        state.borrow_mut().cover_queued.insert(id.clone());
        let tx = state.borrow().refresh_tx.clone();
        std::thread::spawn(move || {
            if appid != 0 {
                let _ = download_cover(appid, &name);
            } else if !url.is_empty() {
                let dest = custom_cover_dest(&id);
                let _ = download_to_file(&url, &dest);
            }
            let _ = tx.send(RefreshMsg::Covers);
        });
    }
}

fn queue_missing_logos(state: &Rc<RefCell<AppState>>) {
    let jobs: Vec<u64> = {
        let s = state.borrow();
        s.steam_games
            .iter()
            .map(|g| g.appid)
            .filter(|id| !s.logo_queued.contains(id) && !logo_local_path(*id).exists())
            .collect()
    };
    for appid in jobs {
        state.borrow_mut().logo_queued.insert(appid);
        let tx = state.borrow().refresh_tx.clone();
        std::thread::spawn(move || {
            let _ = download_logo(appid);
            let _ = tx.send(RefreshMsg::Logos);
        });
    }
}

fn confirm_stop(state: &Rc<RefCell<AppState>>, game_id: &str) {
    let name = state
        .borrow()
        .data
        .games
        .iter()
        .find(|g| g.id == game_id)
        .map(|g| g.name.clone())
        .unwrap_or_else(|| "Игра".to_string());
    let window = state.borrow().window.clone();
    let alert = adw::AlertDialog::new(
        Some(&format!("Завершить «{name}»?")),
        Some("Процесс игры будет принудительно закрыт. Несохранённый прогресс может быть потерян."),
    );
    alert.add_response("cancel", "Отмена");
    alert.add_response("kill", "Завершить");
    alert.set_default_response(Some("cancel"));
    alert.set_close_response("cancel");
    alert.set_response_appearance("kill", adw::ResponseAppearance::Destructive);
    let state_c = state.clone();
    alert.connect_response(None, move |a, resp| {
        if resp == "kill" {
            state_c.borrow_mut().stop_session();
        }
        a.close();
    });
    alert.present(Some(&window));
}

fn build_sidebar(state: &Rc<RefCell<AppState>>) -> gtk::Box {
    let v = gtk::Box::new(Orientation::Vertical, 4);
    v.set_width_request(230);

    let hdr = gtk::Box::new(Orientation::Horizontal, 8);
    hdr.set_margin_top(16);
    hdr.set_margin_bottom(8);
    hdr.set_margin_start(12);
    hdr.set_margin_end(12);
    let icon = gtk::Image::from_icon_name("launchvault");
    icon.set_pixel_size(26);
    let name = gtk::Label::new(Some("LaunchVault"));
    name.add_css_class("title-1");
    name.set_xalign(0.0);
    hdr.append(&icon);
    hdr.append(&name);
    v.append(&hdr);

    let (library_btn, steam_btn, settings_btn) = {
        let s = state.borrow();
        (s.library_btn.clone(), s.steam_btn.clone(), s.settings_btn.clone())
    };
    for b in [&library_btn, &steam_btn, &settings_btn] {
        b.set_hexpand(true);
        b.set_halign(Align::Fill);
        b.set_margin_start(8);
        b.set_margin_end(8);
    }
    library_btn.set_label("Библиотека");
    steam_btn.set_label("Steam");
    settings_btn.set_label("Настройки");
    library_btn.set_active(true);
    steam_btn.set_group(Some(&library_btn));
    settings_btn.set_group(Some(&library_btn));
    v.append(&library_btn);
    v.append(&steam_btn);
    v.append(&settings_btn);

    let sep = gtk::Separator::new(Orientation::Horizontal);
    sep.set_margin_top(8);
    sep.set_margin_bottom(8);
    v.append(&sep);

    let sess_title = gtk::Label::new(Some("Сессия"));
    sess_title.set_xalign(0.0);
    sess_title.add_css_class("heading");
    sess_title.set_margin_start(12);
    v.append(&sess_title);

    let (session_idle, session_game, session_timer) = {
        let s = state.borrow();
        (s.session_idle.clone(), s.session_game.clone(), s.session_timer.clone())
    };
    session_idle.set_xalign(0.0);
    session_idle.set_margin_start(12);
    session_game.set_xalign(0.0);
    session_game.set_margin_start(12);
    session_timer.set_xalign(0.0);
    session_timer.set_margin_start(12);
    session_timer.add_css_class("session-timer");
    v.append(&session_idle);
    v.append(&session_game);
    v.append(&session_timer);

    let session_box = state.borrow().session_box.clone();
    let stop = gtk::Button::with_label("Остановить сессию");
    stop.set_margin_start(12);
    stop.set_margin_top(6);
    let state_c = state.clone();
    stop.connect_clicked(move |_| confirm_stop(&state_c, &state_c.borrow().session.as_ref().map(|s| s.game_id.clone()).unwrap_or_default()));
    session_box.append(&stop);
    v.append(&session_box);

    let spacer = gtk::Box::new(Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    v.append(&spacer);

    let stats = state.borrow().stats_label.clone();
    stats.set_margin_start(12);
    stats.set_margin_end(12);
    stats.set_margin_bottom(12);
    stats.add_css_class("weak-text");
    v.append(&stats);

    v
}

fn build_library_page(state: &Rc<RefCell<AppState>>) -> gtk::Box {
    let page = gtk::Box::new(Orientation::Vertical, 0);

    let hbar = gtk::Box::new(Orientation::Horizontal, 6);
    hbar.set_margin_top(12);
    hbar.set_margin_start(12);
    hbar.set_margin_end(12);
    hbar.set_margin_bottom(4);

    let search = state.borrow().search_entry.clone();
    search.set_placeholder_text(Some("Поиск игры…"));
    search.set_hexpand(true);
    let state_c = state.clone();
    search.connect_search_changed(move |e| {
        state_c.borrow_mut().search = e.text().to_string();
        refresh_library(&state_c);
    });
    hbar.append(&search);

    let add_btn = gtk::Button::with_label("+ Игра");
    add_btn.add_css_class("suggested-action");
    let state_c = state.clone();
    add_btn.connect_clicked(move |_| open_add_game_dialog(&state_c));
    hbar.append(&add_btn);
    page.append(&hbar);

    let flow = state.borrow().library_flow.clone();
    flow.set_halign(Align::Start);
    flow.set_hexpand(true);

    let sw = gtk::ScrolledWindow::new();
    sw.set_vexpand(true);
    sw.set_hexpand(true);
    sw.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    let flow_holder = gtk::Box::new(Orientation::Vertical, 0);
    flow_holder.set_margin_top(8);
    flow_holder.set_margin_bottom(12);
    flow_holder.set_margin_start(12);
    flow_holder.set_margin_end(12);
    flow_holder.append(&flow);
    sw.set_child(Some(&flow_holder));
    page.append(&sw);

    let empty = state.borrow().library_empty.clone();
    let l = gtk::Label::new(Some("Пусто. Добавь первую игру кнопкой «+ Игра»."));
    empty.append(&l);
    empty.set_vexpand(true);
    empty.set_valign(Align::Center);
    empty.set_margin_top(40);
    empty.set_visible(false);
    page.append(&empty);

    page
}

fn open_add_game_dialog(state: &Rc<RefCell<AppState>>) {
    let window = state.borrow().window.clone();
    let dlg = adw::Dialog::builder().title("Новая игра").build();

    let form = gtk::Box::new(Orientation::Vertical, 8);
    form.set_width_request(440);
    form.set_margin_top(12);
    form.set_margin_bottom(12);
    form.set_margin_start(16);
    form.set_margin_end(16);

    let name_entry = gtk::Entry::new();
    name_entry.set_placeholder_text(Some("Название (обязательно)"));
    form.append(&name_entry);

    let path_entry = gtk::Entry::new();
    path_entry.set_placeholder_text(Some("Путь к запуску или команда"));
    let path_browse = gtk::Button::with_label("Выбрать…");
    let fc = gtk::FileDialog::new();
    fc.set_title("Выбрать файл игры");
    let window_c = window.clone();
    let pe = path_entry.clone();
    path_browse.connect_clicked(move |_| {
        let pe = pe.clone();
        fc.clone().open(Some(&window_c), None::<&gtk::gio::Cancellable>, move |res| {
            if let Ok(file) = res {
                if let Some(p) = file.path() {
                    pe.set_text(&p.to_string_lossy());
                }
            }
        });
    });
    let path_row = gtk::Box::new(Orientation::Horizontal, 6);
    path_row.append(&path_entry);
    path_row.append(&path_browse);
    form.append(&path_row);

    let args_entry = gtk::Entry::new();
    args_entry.set_placeholder_text(Some("Аргументы (необязательно)"));
    form.append(&args_entry);

    let cover_entry = gtk::Entry::new();
    cover_entry.set_placeholder_text(Some("Обложка: файл или http://…"));
    let cover_browse = gtk::Button::with_label("Выбрать…");
    let fc2 = gtk::FileDialog::new();
    fc2.set_title("Выбрать обложку");
    let window_c = window.clone();
    let ce = cover_entry.clone();
    cover_browse.connect_clicked(move |_| {
        let ce = ce.clone();
        fc2.clone().open(Some(&window_c), None::<&gtk::gio::Cancellable>, move |res| {
            if let Ok(file) = res {
                if let Some(p) = file.path() {
                    ce.set_text(&p.to_string_lossy());
                }
            }
        });
    });
    let cover_row = gtk::Box::new(Orientation::Horizontal, 6);
    cover_row.append(&cover_entry);
    cover_row.append(&cover_browse);
    form.append(&cover_row);

    let notes_view = gtk::TextView::new();
    notes_view.set_wrap_mode(gtk::WrapMode::WordChar);
    notes_view.set_height_request(72);
    let notes_sw = gtk::ScrolledWindow::new();
    notes_sw.set_child(Some(&notes_view));
    notes_sw.set_height_request(72);
    form.append(&notes_sw);

    let err = gtk::Label::new(Some("Название обязательно"));
    err.add_css_class("error");
    err.set_visible(false);
    form.append(&err);

    let buttons = gtk::Box::new(Orientation::Horizontal, 6);
    buttons.set_halign(Align::End);
    let cancel = gtk::Button::with_label("Отмена");
    let dlg_c = dlg.clone();
    cancel.connect_clicked(move |_| {
        dlg_c.close();
    });
    buttons.append(&cancel);
    let ok = gtk::Button::with_label("Добавить");
    ok.add_css_class("suggested-action");
    buttons.append(&ok);
    form.append(&buttons);

    dlg.set_child(Some(&form));
    let dlg_c = dlg.clone();
    let state_c = state.clone();
    ok.connect_clicked(move |_| {
        let name = name_entry.text().trim().to_string();
        if name.is_empty() {
            err.set_visible(true);
            name_entry.grab_focus();
            return;
        }
        let path = path_entry.text().trim().to_string();
        let args = args_entry.text().trim().to_string();
        let cover = cover_entry.text().trim().to_string();
        let notes = {
            let (s, e) = notes_view.buffer().bounds();
            notes_view.buffer().text(&s, &e, false).trim().to_string()
        };
        {
            let mut st = state_c.borrow_mut();
            st.data.games.push(Game {
                id: uuid::Uuid::new_v4().to_string(),
                name,
                source: Source::Local,
                steam_id: None,
                path: if path.is_empty() { None } else { Some(path) },
                args: if args.is_empty() { None } else { Some(args) },
                cover: if cover.is_empty() { None } else { Some(cover) },
                notes: if notes.is_empty() { None } else { Some(notes) },
                playtime_sec: 0,
                last_played: None,
                added_at: now_secs(),
            });
            st.data
                .games
                .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            st.save();
        }
        refresh_library(&state_c);
        dlg_c.close();
    });

    dlg.present(Some(&window));
}

fn build_steam_page(state: &Rc<RefCell<AppState>>) -> gtk::Box {
    let page = gtk::Box::new(Orientation::Vertical, 0);

    let top = gtk::Box::new(Orientation::Vertical, 6);
    top.set_margin_top(12);
    top.set_margin_start(12);
    top.set_margin_end(12);
    top.set_margin_bottom(8);

    let path_row = gtk::Box::new(Orientation::Horizontal, 6);
    let path_entry = gtk::Entry::new();
    path_entry.set_hexpand(true);
    let stored = state.borrow().data.steam_path.clone().unwrap_or_default();
    path_entry.set_text(&stored);
    path_entry.set_placeholder_text(Some("Путь к Steam"));
    path_row.append(&path_entry);

    let browse = gtk::Button::with_label("Выбрать…");
    let fc = gtk::FileDialog::new();
    fc.set_title("Выбрать папку Steam");
    let window = state.borrow().window.clone();
    let pe = path_entry.clone();
    browse.connect_clicked(move |_| {
        let pe = pe.clone();
        fc.clone().select_folder(Some(&window), None::<&gtk::gio::Cancellable>, move |res| {
            if let Ok(folder) = res {
                if let Some(p) = folder.path() {
                    pe.set_text(&p.to_string_lossy());
                }
            }
        });
    });
    path_row.append(&browse);

    let sync = gtk::Button::with_label("Синхронизировать");
    sync.add_css_class("suggested-action");
    let state_c = state.clone();
    let pe = path_entry.clone();
    sync.connect_clicked(move |_| {
        let p = pe.text().trim().to_string();
        let root = PathBuf::from(&p);
        let mut s = state_c.borrow_mut();
        if root.join("steamapps").is_dir() {
            s.data.steam_path = Some(p);
            s.steam_games = installed_games(&root);
            s.steam_error = None;
            s.save();
            drop(s);
            refresh_steam(&state_c);
        } else {
            s.steam_error = Some("Путь не найден или не содержит steamapps".to_string());
            drop(s);
            refresh_steam(&state_c);
        }
    });
    path_row.append(&sync);
    top.append(&path_row);

    let hint = state.borrow().steam_hint.clone();
    hint.set_xalign(0.0);
    hint.add_css_class("weak-text");
    top.append(&hint);
    page.append(&top);

    let list = state.borrow().steam_list.clone();
    list.set_selection_mode(gtk::SelectionMode::None);
    let sw = gtk::ScrolledWindow::new();
    sw.set_vexpand(true);
    sw.set_hexpand(true);
    sw.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    let holder = gtk::Box::new(Orientation::Vertical, 0);
    holder.set_margin_top(4);
    holder.set_margin_start(8);
    holder.set_margin_end(8);
    holder.append(&list);
    sw.set_child(Some(&holder));
    page.append(&sw);

    page
}

fn build_settings_page(state: &Rc<RefCell<AppState>>) -> gtk::Box {
    let page = gtk::Box::new(Orientation::Vertical, 0);
    let sw = gtk::ScrolledWindow::new();
    sw.set_vexpand(true);
    sw.set_hexpand(true);
    sw.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

    let v = gtk::Box::new(Orientation::Vertical, 12);
    v.set_margin_top(12);
    v.set_margin_bottom(16);
    v.set_margin_start(16);
    v.set_margin_end(16);
    v.set_width_request(640);
    v.set_halign(Align::Center);

    let umu_group = adw::PreferencesGroup::builder()
        .title("UMU / Proton")
        .description("Запуск Windows-игр (.exe) через umu-launcher и Proton — как в Steam, но вне Steam.")
        .build();

    let umu_enabled = adw::SwitchRow::builder()
        .title("Использовать UMU для .exe-игр")
        .build();
    umu_enabled.set_active(state.borrow().data.umu.enabled);
    umu_group.add(&umu_enabled);

    let umu_run_entry = gtk::Entry::new();
    umu_run_entry.set_text(&state.borrow().data.umu.umu_run);
    umu_run_entry.set_placeholder_text(Some("umu-run"));
    umu_run_entry.set_width_chars(20);
    let umu_find = gtk::Button::with_label("Найти");
    let e = umu_run_entry.clone();
    umu_find.connect_clicked(move |_| {
        if let Some(p) = find_umu_run() {
            e.set_text(&p.to_string_lossy());
        }
    });
    let umu_run_row = adw::ActionRow::builder().title("umu-run").build();
    umu_run_row.add_suffix(&umu_find);
    umu_run_row.add_suffix(&umu_run_entry);
    umu_group.add(&umu_run_row);

    let proton_entry = gtk::Entry::new();
    proton_entry.set_text(&state.borrow().data.umu.proton_path);
    proton_entry.set_placeholder_text(Some("путь к Proton (или GE-Proton)"));
    proton_entry.set_width_chars(20);
    let proton_find = gtk::Button::with_label("Найти");
    let e = proton_entry.clone();
    proton_find.connect_clicked(move |_| {
        if let Some(p) = find_proton_root() {
            e.set_text(&p.to_string_lossy());
        }
    });
    let proton_row = adw::ActionRow::builder().title("Proton").build();
    proton_row.add_suffix(&proton_find);
    proton_row.add_suffix(&proton_entry);
    umu_group.add(&proton_row);

    let store_entry = gtk::Entry::new();
    store_entry.set_text(&state.borrow().data.umu.store);
    store_entry.set_placeholder_text(Some("steam / egs / none"));
    store_entry.set_width_chars(20);
    let store_row = adw::ActionRow::builder().title("STORE").build();
    store_row.add_suffix(&store_entry);
    umu_group.add(&store_row);

    let game_id_entry = gtk::Entry::new();
    game_id_entry.set_text(&state.borrow().data.umu.game_id);
    game_id_entry.set_placeholder_text(Some("пусто = сгенерировать автоматически"));
    game_id_entry.set_width_chars(20);
    let game_id_row = adw::ActionRow::builder().title("GAMEID").build();
    game_id_row.add_suffix(&game_id_entry);
    umu_group.add(&game_id_row);

    let wineprefix_entry = gtk::Entry::new();
    wineprefix_entry.set_text(&state.borrow().data.umu.wineprefix);
    wineprefix_entry.set_placeholder_text(Some("пусто = ~/Games/umu/<GAMEID>"));
    wineprefix_entry.set_width_chars(20);
    let wineprefix_row = adw::ActionRow::builder().title("WINEPREFIX").build();
    wineprefix_row.add_suffix(&wineprefix_entry);
    umu_group.add(&wineprefix_row);

    let save_btn = gtk::Button::with_label("Сохранить");
    save_btn.add_css_class("suggested-action");
    save_btn.set_halign(Align::End);
    let state_c = state.clone();
    save_btn.connect_clicked(move |_| {
        let mut s = state_c.borrow_mut();
        s.data.umu.enabled = umu_enabled.is_active();
        s.data.umu.umu_run = umu_run_entry.text().trim().to_string();
        s.data.umu.proton_path = proton_entry.text().trim().to_string();
        s.data.umu.store = store_entry.text().trim().to_string();
        s.data.umu.game_id = game_id_entry.text().trim().to_string();
        s.data.umu.wineprefix = wineprefix_entry.text().trim().to_string();
        s.save();
        drop(s);
        let toast = adw::Toast::new("Настройки UMU сохранены");
        state_c.borrow().toast.add_toast(toast);
    });

    let steam_group = adw::PreferencesGroup::builder()
        .title("Steam")
        .description("Путь к установленному Steam (можно найти автоматически).")
        .build();
    let steam_auto = gtk::Button::with_label("Автоопределить");
    let state_c = state.clone();
    steam_auto.connect_clicked(move |_| {
        let mut s = state_c.borrow_mut();
        if let Some(p) = find_steam_root() {
            s.data.steam_path = Some(p.to_string_lossy().into_owned());
            s.steam_games = installed_games(&p);
            s.steam_error = None;
            s.save();
            drop(s);
            refresh_steam(&state_c);
        } else {
            s.steam_error = Some("Steam не найден".to_string());
            drop(s);
            refresh_steam(&state_c);
        }
    });
    let steam_row = adw::ActionRow::builder()
        .title("Путь к Steam")
        .subtitle(&state.borrow().data.steam_path.clone().unwrap_or_else(|| "не задан".into()))
        .build();
    steam_row.add_suffix(&steam_auto);
    steam_group.add(&steam_row);

    let data_group = adw::PreferencesGroup::builder().title("Данные").build();
    let data_row = adw::ActionRow::builder()
        .title("Папка данных")
        .subtitle(app_data_dir().display().to_string())
        .build();
    data_group.add(&data_row);
    let covers_row = adw::ActionRow::builder()
        .title("Обложки")
        .subtitle(covers_dir().display().to_string())
        .build();
    data_group.add(&covers_row);
    let clear_btn = gtk::Button::with_label("Очистить кэш");
    let state_c = state.clone();
    clear_btn.connect_clicked(move |_| {
        let mut s = state_c.borrow_mut();
        let _ = std::fs::remove_dir_all(covers_dir());
        let _ = std::fs::create_dir_all(covers_dir());
        s.cover_queued.clear();
        s.logo_queued.clear();
        s.save();
        drop(s);
        refresh_library(&state_c);
        refresh_steam(&state_c);
    });
    let clear_row = adw::ActionRow::builder().title("Кэш обложек").build();
    clear_row.add_suffix(&clear_btn);
    data_group.add(&clear_row);

    v.append(&umu_group);
    v.append(&save_btn);
    v.append(&steam_group);
    v.append(&data_group);

    let about = gtk::Label::new(Some(&format!(
        "LaunchVault {} · © 2026 kik4311 · GPL-3.0",
        env!("CARGO_PKG_VERSION")
    )));
    about.add_css_class("weak-text");
    about.set_halign(Align::Center);
    v.append(&about);

    sw.set_child(Some(&v));
    page.append(&sw);
    page
}

fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn build_window(app: &gtk::Application) {
    load_css();
    storage::ensure_dirs();
    let mut data = storage::load();
    let (mut steam_games, mut steam_error) = (Vec::new(), None);
    let root = data
        .steam_path
        .as_deref()
        .map(PathBuf::from)
        .or_else(find_steam_root);
    match root {
        Some(p) => {
            data.steam_path = Some(p.to_string_lossy().into_owned());
            steam_games = installed_games(&p);
        }
        None => steam_error = Some("Steam не найден — укажи путь на странице Steam".to_string()),
    }

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("LaunchVault")
        .default_width(1280)
        .default_height(800)
        .build();
    window.set_icon_name(Some("launchvault"));

    let nav = adw::NavigationView::new();
    let stack = gtk::Stack::new();
    let root_page = adw::NavigationPage::new(&stack, "Библиотека");
    nav.add(&root_page);

    let (refresh_tx, refresh_rx) = std::sync::mpsc::channel::<RefreshMsg>();

    let state = Rc::new(RefCell::new(AppState {
        data,
        steam_games,
        steam_error,
        session: None,
        page: Page::Library,
        search: String::new(),
        cover_queued: HashSet::new(),
        logo_queued: HashSet::new(),
        last_save: now_secs(),
        window: window.clone(),
        toast: adw::ToastOverlay::new(),
        nav: nav.clone(),
        root_page: root_page.clone(),
        stack: stack.clone(),
        library_flow: gtk::Box::new(Orientation::Vertical, 12),
        library_empty: gtk::Box::new(Orientation::Vertical, 0),
        library_btn: gtk::ToggleButton::new(),
        steam_btn: gtk::ToggleButton::new(),
        settings_btn: gtk::ToggleButton::new(),
        session_box: gtk::Box::new(Orientation::Vertical, 4),
        session_idle: gtk::Label::new(Some("(ничего)")),
        session_game: gtk::Label::new(None),
        session_timer: gtk::Label::new(None),
        stats_label: gtk::Label::new(None),
        steam_list: gtk::ListBox::new(),
        steam_hint: gtk::Label::new(None),
        search_entry: gtk::SearchEntry::new(),
        refresh_tx,
        back_btn: gtk::Button::new(),
    }));

    let sidebar = build_sidebar(&state);
    let sidebar_page = adw::NavigationPage::new(&sidebar, "LaunchVault");

    let library_page = build_library_page(&state);
    let steam_page = build_steam_page(&state);
    let settings_page = build_settings_page(&state);
    stack.add_named(&library_page, Some("library"));
    stack.add_named(&steam_page, Some("steam"));
    stack.add_named(&settings_page, Some("settings"));

    let content_page = adw::NavigationPage::new(&nav, "LaunchVault");
    let split = adw::NavigationSplitView::new();
    split.set_sidebar(Some(&sidebar_page));
    split.set_content(Some(&content_page));
    split.set_sidebar_width_fraction(0.22);

    let toast = state.borrow().toast.clone();
    toast.set_child(Some(&split));

    let header = adw::HeaderBar::new();

    let back_btn = state.borrow().back_btn.clone();
    back_btn.set_icon_name("go-previous-symbolic");
    back_btn.set_tooltip_text(Some("Назад"));
    back_btn.set_visible(false);
    header.pack_start(&back_btn);

    let nav_c = nav.clone();
    back_btn.connect_clicked(move |_| {
        nav_c.pop();
    });

    let back_btn_c = back_btn.clone();
    let root_c = root_page.clone();
    nav.connect_visible_page_notify(move |n| {
        back_btn_c.set_visible(n.visible_page().as_ref() != Some(&root_c));
    });

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&toast));
    window.set_content(Some(&toolbar));

    let state_c = state.clone();
    let lib_btn = state.borrow().library_btn.clone();
    lib_btn.connect_toggled(move |b| {
        if b.is_active() {
            let mut s = state_c.borrow_mut();
            s.switch_page(Page::Library);
            drop(s);
            refresh_library(&state_c);
        }
    });
    let state_c = state.clone();
    let st_btn = state.borrow().steam_btn.clone();
    st_btn.connect_toggled(move |b| {
        if b.is_active() {
            let mut s = state_c.borrow_mut();
            s.switch_page(Page::Steam);
            drop(s);
            refresh_steam(&state_c);
        }
    });
    let state_c = state.clone();
    let se_btn = state.borrow().settings_btn.clone();
    se_btn.connect_toggled(move |b| {
        if b.is_active() {
            state_c.borrow_mut().switch_page(Page::Settings);
        }
    });

    let state_c = state.clone();
    window.connect_close_request(move |_| {
        state_c.borrow_mut().save();
        glib::Propagation::Proceed
    });

    refresh_library(&state);
    refresh_steam(&state);
    state.borrow_mut().update_session_ui();

    let state_c = state.clone();
    glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
        loop {
            match refresh_rx.try_recv() {
                Ok(RefreshMsg::Covers) => refresh_library(&state_c),
                Ok(RefreshMsg::Logos) => refresh_steam(&state_c),
                Err(_) => break,
            }
        }
        let mut s = state_c.borrow_mut();
        s.update_session_ui();
        let now = now_secs();
        if now.saturating_sub(s.last_save) >= 30 {
            s.last_save = now;
            s.save();
        }
        ControlFlow::Continue
    });

    window.present();
}

#[derive(Clone, Copy, Debug)]
enum RefreshMsg {
    Covers,
    Logos,
}

pub fn run() {
    adw::init().expect("Не удалось инициализировать libadwaita");
    let app = gtk::Application::builder()
        .application_id("org.launchvault.LaunchVault")
        .build();
    app.connect_activate(build_window);
    let _ = app.run();
}
