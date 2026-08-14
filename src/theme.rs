use eframe::egui::style::{Selection, WidgetVisuals};
use eframe::egui::{Color32, CornerRadius, Stroke, Visuals};

/// Выбранная пользователем тема (базовый стиль).
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeKind {
    Auto,
    Adwaita,
    Breeze,
    Windows11,
}

impl ThemeKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Авто (как в системе)",
            Self::Adwaita => "Adwaita (GNOME)",
            Self::Breeze => "Breeze (KDE)",
            Self::Windows11 => "Windows 11",
        }
    }

    pub fn all() -> [Self; 4] {
        [Self::Auto, Self::Adwaita, Self::Breeze, Self::Windows11]
    }

    pub fn from_name(s: &str) -> Self {
        match s {
            "adwaita" => Self::Adwaita,
            "breeze" => Self::Breeze,
            "windows11" => Self::Windows11,
            _ => Self::Auto,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Adwaita => "adwaita",
            Self::Breeze => "breeze",
            Self::Windows11 => "windows11",
        }
    }

    pub fn detect(self, dark: bool) -> ThemePreset {
        match self {
            Self::Auto => system_preset(dark),
            Self::Adwaita => {
                if dark {
                    ThemePreset::AdwaitaDark
                } else {
                    ThemePreset::AdwaitaLight
                }
            }
            Self::Breeze => {
                if dark {
                    ThemePreset::BreezeDark
                } else {
                    ThemePreset::BreezeLight
                }
            }
            Self::Windows11 => {
                if dark {
                    ThemePreset::Windows11Dark
                } else {
                    ThemePreset::Windows11Light
                }
            }
        }
    }
}

/// Готовый набор цветов, применяемый к egui.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemePreset {
    AdwaitaDark,
    AdwaitaLight,
    BreezeDark,
    BreezeLight,
    Windows11Dark,
    Windows11Light,
}

impl ThemePreset {
    pub fn dark(self) -> bool {
        matches!(
            self,
            Self::AdwaitaDark | Self::BreezeDark | Self::Windows11Dark
        )
    }

    pub fn is_windows(self) -> bool {
        matches!(self, Self::Windows11Dark | Self::Windows11Light)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::AdwaitaDark => "Adwaita (тёмная)",
            Self::AdwaitaLight => "Adwaita (светлая)",
            Self::BreezeDark => "Breeze (тёмная)",
            Self::BreezeLight => "Breeze (светлая)",
            Self::Windows11Dark => "Windows 11 (тёмная)",
            Self::Windows11Light => "Windows 11 (светлая)",
        }
    }
}

pub fn system_preset(dark: bool) -> ThemePreset {
    #[cfg(target_os = "windows")]
    {
        let _ = dark;
        if dark {
            ThemePreset::Windows11Dark
        } else {
            ThemePreset::Windows11Light
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let de = std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_uppercase();
        let kde = de.contains("KDE") || de.contains("PLASMA");
        match (kde, dark) {
            (true, true) => ThemePreset::BreezeDark,
            (true, false) => ThemePreset::BreezeLight,
            (false, true) => ThemePreset::AdwaitaDark,
            (false, false) => ThemePreset::AdwaitaLight,
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = dark;
        if dark {
            ThemePreset::AdwaitaDark
        } else {
            ThemePreset::AdwaitaLight
        }
    }
    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        all(unix, not(target_os = "macos"))
    )))]
    {
        let _ = dark;
        ThemePreset::AdwaitaDark
    }
}

struct Palette {
    bg: Color32,
    base: Color32,
    fg: Color32,
    accent: Color32,
    accent_fg: Color32,
    border: Color32,
    selected: Color32,
    warn: Color32,
    error: Color32,
}

fn palette(p: ThemePreset) -> Palette {
    let rgb = |r: u8, g: u8, b: u8| Color32::from_rgb(r, g, b);
    let p = match p {
        ThemePreset::AdwaitaDark => Palette {
            bg: rgb(0x1e, 0x1e, 0x1e),
            base: rgb(0x2d, 0x2d, 0x2d),
            fg: rgb(0xff, 0xff, 0xff),
            accent: rgb(0x78, 0xae, 0xed),
            accent_fg: rgb(0x0c, 0x0c, 0x0c),
            border: rgb(0x3b, 0x3b, 0x3b),
            selected: rgb(0x35, 0x84, 0xe4),
            warn: rgb(0xff, 0xc0, 0x4d),
            error: rgb(0xed, 0x73, 0x73),
        },
        ThemePreset::AdwaitaLight => Palette {
            bg: rgb(0xf6, 0xf5, 0xf4),
            base: rgb(0xff, 0xff, 0xff),
            fg: rgb(0x20, 0x20, 0x20),
            accent: rgb(0x35, 0x84, 0xe4),
            accent_fg: rgb(0xff, 0xff, 0xff),
            border: rgb(0xe0, 0xdf, 0xdd),
            selected: rgb(0x35, 0x84, 0xe4),
            warn: rgb(0xd0, 0x86, 0x00),
            error: rgb(0xc0, 0x1c, 0x28),
        },
        ThemePreset::BreezeDark => Palette {
            bg: rgb(0x31, 0x36, 0x3b),
            base: rgb(0x23, 0x26, 0x29),
            fg: rgb(0xfc, 0xfc, 0xfc),
            accent: rgb(0x3d, 0xae, 0xe9),
            accent_fg: rgb(0x0a, 0x0a, 0x0a),
            border: rgb(0x41, 0x47, 0x4c),
            selected: rgb(0x3d, 0xae, 0xe9),
            warn: rgb(0xfc, 0xe9, 0x4f),
            error: rgb(0xe2, 0x8f, 0x8f),
        },
        ThemePreset::BreezeLight => Palette {
            bg: rgb(0xef, 0xf0, 0xf1),
            base: rgb(0xff, 0xff, 0xff),
            fg: rgb(0x23, 0x26, 0x29),
            accent: rgb(0x1d, 0x99, 0xf3),
            accent_fg: rgb(0xff, 0xff, 0xff),
            border: rgb(0xd7, 0xd9, 0xdc),
            selected: rgb(0x3d, 0xae, 0xe9),
            warn: rgb(0xa0, 0x7f, 0x00),
            error: rgb(0xc8, 0x32, 0x32),
        },
        ThemePreset::Windows11Dark => Palette {
            bg: rgb(0x20, 0x20, 0x20),
            base: rgb(0x2b, 0x2b, 0x2b),
            fg: rgb(0xff, 0xff, 0xff),
            accent: rgb(0x4c, 0xc2, 0xff),
            accent_fg: rgb(0x0c, 0x0c, 0x0c),
            border: rgb(0x3b, 0x3b, 0x3b),
            selected: rgb(0x4c, 0xa8, 0xe0),
            warn: rgb(0xff, 0xc0, 0x4d),
            error: rgb(0xed, 0x6a, 0x6a),
        },
        ThemePreset::Windows11Light => Palette {
            bg: rgb(0xf3, 0xf3, 0xf3),
            base: rgb(0xfa, 0xfa, 0xfa),
            fg: rgb(0x1a, 0x1a, 0x1a),
            accent: rgb(0x00, 0x78, 0xd4),
            accent_fg: rgb(0xff, 0xff, 0xff),
            border: rgb(0xe5, 0xe5, 0xe5),
            selected: rgb(0x00, 0x78, 0xd4),
            warn: rgb(0x9d, 0x5d, 0x00),
            error: rgb(0xc4, 0x2b, 0x1c),
        },
    };
    p
}

pub fn accent_color(p: ThemePreset) -> Color32 {
    palette(p).accent
}

pub fn visuals(p: ThemePreset) -> Visuals {
    let pal = palette(p);
    let dark = p.dark();

    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    let window_fill = if p.is_windows() {
        pal.base.gamma_multiply_u8(225)
    } else {
        pal.base
    };
    let panel_fill = if p.is_windows() {
        pal.bg.gamma_multiply_u8(220)
    } else {
        pal.bg
    };

    let hover = pal.accent.lerp_to_gamma(pal.base, 0.30);
    let faint = if dark {
        pal.base.lerp_to_gamma(pal.fg, 0.05)
    } else {
        pal.base.lerp_to_gamma(pal.bg, 0.35)
    };
    let extreme = if dark {
        pal.base.lerp_to_gamma(pal.fg, 0.10)
    } else {
        pal.bg
    };

    v.dark_mode = dark;
    v.override_text_color = Some(pal.fg);
    v.panel_fill = panel_fill;
    v.window_fill = window_fill;
    v.extreme_bg_color = extreme;
    v.faint_bg_color = faint;
    v.code_bg_color = faint;
    v.warn_fg_color = pal.warn;
    v.error_fg_color = pal.error;
    v.hyperlink_color = pal.accent;
    v.selection = Selection {
        bg_fill: pal.selected,
        stroke: Stroke::new(1.0, pal.fg),
    };
    v.window_stroke = Stroke::new(1.0, pal.border);
    v.window_corner_radius = CornerRadius::same(8);
    v.menu_corner_radius = CornerRadius::same(8);

    let wv = |bg_fill: Color32,
              weak: Color32,
              bg_stroke: Stroke,
              fg_stroke: Stroke,
              radius: u8| WidgetVisuals {
        bg_fill,
        weak_bg_fill: weak,
        bg_stroke,
        corner_radius: CornerRadius::same(radius),
        fg_stroke,
        expansion: 0.0,
    };

    v.widgets.noninteractive = wv(
        pal.base,
        pal.base,
        Stroke::new(0.0, pal.border),
        Stroke::new(1.0, pal.fg),
        6,
    );
    v.widgets.inactive = wv(
        pal.base,
        pal.base,
        Stroke::new(1.0, pal.border),
        Stroke::new(1.0, pal.fg),
        6,
    );
    v.widgets.hovered = wv(
        hover,
        hover,
        Stroke::new(1.0, pal.accent),
        Stroke::new(1.0, pal.fg),
        6,
    );
    v.widgets.active = wv(
        pal.accent,
        pal.accent,
        Stroke::new(1.0, pal.accent),
        Stroke::new(1.0, pal.accent_fg),
        6,
    );
    v.widgets.open = wv(
        hover,
        hover,
        Stroke::new(1.0, pal.accent),
        Stroke::new(1.0, pal.fg),
        6,
    );

    v
}
