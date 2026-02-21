/// Theme system — switchable color themes for the application
///
/// Provides a Light ("Scientific") and a Dark ("Cyberpunk") theme.

/// Available themes
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AppTheme {
    Light,
    Cyberpunk,
}

impl AppTheme {
    pub fn label(&self) -> &'static str {
        match self {
            AppTheme::Light => "☀ Light",
            AppTheme::Cyberpunk => "Neon Dark SLAY",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            AppTheme::Light => AppTheme::Cyberpunk,
            AppTheme::Cyberpunk => AppTheme::Light,
        }
    }
}

/// All colors a theme needs to provide
#[derive(Debug, Clone)]
pub struct ThemeColors {
    // Panels & backgrounds
    pub panel_fill: egui::Color32,
    pub window_fill: egui::Color32,
    pub faint_bg: egui::Color32,

    // Widgets
    pub widget_bg: egui::Color32,
    pub widget_bg_stroke: egui::Color32,
    pub widget_inactive_bg: egui::Color32,
    pub widget_inactive_stroke: egui::Color32,
    pub widget_hovered_bg: egui::Color32,
    pub widget_hovered_stroke: egui::Color32,
    pub widget_active_bg: egui::Color32,
    pub widget_active_fg: egui::Color32,

    // Selection
    pub selection_bg: egui::Color32,
    pub selection_stroke: egui::Color32,

    // Text
    pub text_primary: egui::Color32,
    pub text_secondary: egui::Color32,
    pub text_muted: egui::Color32,
    pub text_heading: egui::Color32,

    // Accent colors
    pub accent: egui::Color32,
    pub accent_dim: egui::Color32,
    pub success: egui::Color32,
    pub warning: egui::Color32,
    pub error: egui::Color32,

    // Spectrum plot
    pub spectrum_line: egui::Color32,
    pub spectrum_phase: egui::Color32,
    pub spectrum_imaginary: egui::Color32,
    pub peak_marker: egui::Color32,
    pub peak_label: egui::Color32,
    pub multiplet_label: egui::Color32,
    pub integration_colors: [egui::Color32; 4],
    pub j_coupling_color: egui::Color32,
    pub baseline_marker: egui::Color32,

    // Tab buttons
    pub tab_active_bg: egui::Color32,
    pub tab_active_text: egui::Color32,
    pub tab_inactive_bg: egui::Color32,
    pub tab_inactive_text: egui::Color32,

    // Status bar
    pub status_bar_bg: egui::Color32,
    pub status_text: egui::Color32,

    // Mode indicator
    pub mode_picking_bg: egui::Color32,
    pub mode_picking_text: egui::Color32,

    // Shadow
    pub shadow_color: egui::Color32,

    // Whether this is a dark theme
    pub is_dark: bool,
}

impl ThemeColors {
    pub fn from_theme(theme: AppTheme) -> Self {
        match theme {
            AppTheme::Light => Self::light(),
            AppTheme::Cyberpunk => Self::cyberpunk(),
        }
    }

    fn light() -> Self {
        Self {
            panel_fill: egui::Color32::from_rgb(0xF7, 0xF7, 0xF8),
            window_fill: egui::Color32::from_rgb(0xFF, 0xFF, 0xFF),
            faint_bg: egui::Color32::from_rgb(0xF0, 0xF1, 0xF3),

            widget_bg: egui::Color32::from_rgb(0xEB, 0xEC, 0xEE),
            widget_bg_stroke: egui::Color32::from_rgb(0xD0, 0xD2, 0xD6),
            widget_inactive_bg: egui::Color32::from_rgb(0xE3, 0xE5, 0xE8),
            widget_inactive_stroke: egui::Color32::from_rgb(0xC8, 0xCA, 0xCE),
            widget_hovered_bg: egui::Color32::from_rgb(0xD8, 0xDD, 0xE6),
            widget_hovered_stroke: egui::Color32::from_rgb(0x5B, 0x9B, 0xD5),
            widget_active_bg: egui::Color32::from_rgb(0x3B, 0x7D, 0xC0),
            widget_active_fg: egui::Color32::WHITE,

            selection_bg: egui::Color32::from_rgba_premultiplied(0x3B, 0x7D, 0xC0, 0x40),
            selection_stroke: egui::Color32::from_rgb(0x3B, 0x7D, 0xC0),

            text_primary: egui::Color32::from_rgb(0x2A, 0x2E, 0x36),
            text_secondary: egui::Color32::from_rgb(0x44, 0x48, 0x52),
            text_muted: egui::Color32::from_rgb(0x88, 0x8C, 0x94),
            text_heading: egui::Color32::from_rgb(0x2A, 0x2E, 0x36),

            accent: egui::Color32::from_rgb(0x3B, 0x7D, 0xC0),
            accent_dim: egui::Color32::from_rgb(0x70, 0x75, 0x80),
            success: egui::Color32::from_rgb(0x27, 0x8B, 0x4A),
            warning: egui::Color32::from_rgb(0xB8, 0x8B, 0x00),
            error: egui::Color32::from_rgb(0xD0, 0x30, 0x30),

            spectrum_line: egui::Color32::from_rgb(0x1A, 0x47, 0x80),
            spectrum_phase: egui::Color32::from_rgb(0x27, 0x8B, 0x4A),
            spectrum_imaginary: egui::Color32::from_rgb(0xD4, 0x55, 0x45),
            peak_marker: egui::Color32::from_rgb(0xD0, 0x30, 0x30),
            peak_label: egui::Color32::from_rgb(0xA0, 0x20, 0x20),
            multiplet_label: egui::Color32::from_rgb(0x20, 0x50, 0xA0),
            integration_colors: [
                egui::Color32::from_rgba_premultiplied(0x40, 0x80, 0xC0, 0x35),
                egui::Color32::from_rgba_premultiplied(0xC0, 0x60, 0x40, 0x35),
                egui::Color32::from_rgba_premultiplied(0x40, 0xA0, 0x60, 0x35),
                egui::Color32::from_rgba_premultiplied(0x90, 0x40, 0xC0, 0x35),
            ],
            j_coupling_color: egui::Color32::from_rgb(0xCC, 0x66, 0x00),
            baseline_marker: egui::Color32::from_rgb(0x60, 0x60, 0x60),

            tab_active_bg: egui::Color32::from_rgb(0x3B, 0x7D, 0xC0),
            tab_active_text: egui::Color32::WHITE,
            tab_inactive_bg: egui::Color32::from_rgb(0xE8, 0xEA, 0xED),
            tab_inactive_text: egui::Color32::from_rgb(0x55, 0x58, 0x62),

            status_bar_bg: egui::Color32::from_rgb(0xF0, 0xF1, 0xF3),
            status_text: egui::Color32::from_rgb(0x44, 0x48, 0x52),

            mode_picking_bg: egui::Color32::from_rgba_premultiplied(0xFF, 0xE0, 0x40, 0x50),
            mode_picking_text: egui::Color32::from_rgb(0x80, 0x40, 0x00),

            shadow_color: egui::Color32::from_rgba_premultiplied(0, 0, 0, 25),

            is_dark: false,
        }
    }

    fn cyberpunk() -> Self {
        Self {
            // Deep dark backgrounds with slight purple/blue tint
            panel_fill: egui::Color32::from_rgb(0x0D, 0x0B, 0x1A),
            window_fill: egui::Color32::from_rgb(0x12, 0x10, 0x22),
            faint_bg: egui::Color32::from_rgb(0x16, 0x14, 0x28),

            // Widgets: dark with neon edges
            widget_bg: egui::Color32::from_rgb(0x1A, 0x18, 0x2E),
            widget_bg_stroke: egui::Color32::from_rgb(0x3A, 0x28, 0x5C),
            widget_inactive_bg: egui::Color32::from_rgb(0x1E, 0x1C, 0x34),
            widget_inactive_stroke: egui::Color32::from_rgb(0x44, 0x30, 0x6A),
            widget_hovered_bg: egui::Color32::from_rgb(0x28, 0x20, 0x44),
            widget_hovered_stroke: egui::Color32::from_rgb(0x00, 0xFF, 0xE0), // neon cyan
            widget_active_bg: egui::Color32::from_rgb(0xFF, 0x00, 0x8C),      // hot pink
            widget_active_fg: egui::Color32::WHITE,

            // Selection: neon purple
            selection_bg: egui::Color32::from_rgba_premultiplied(0xBD, 0x00, 0xFF, 0x40),
            selection_stroke: egui::Color32::from_rgb(0xBD, 0x00, 0xFF),

            // Text: bright on dark
            text_primary: egui::Color32::from_rgb(0xE0, 0xE0, 0xF0),
            text_secondary: egui::Color32::from_rgb(0xA0, 0x9E, 0xB8),
            text_muted: egui::Color32::from_rgb(0x6A, 0x68, 0x80),
            text_heading: egui::Color32::from_rgb(0x00, 0xFF, 0xE0), // neon cyan headings

            // Accents: neon
            accent: egui::Color32::from_rgb(0xFF, 0x00, 0x8C),       // hot pink
            accent_dim: egui::Color32::from_rgb(0x8B, 0x5C, 0xF6),   // purple
            success: egui::Color32::from_rgb(0x00, 0xFF, 0x88),       // neon green
            warning: egui::Color32::from_rgb(0xFF, 0xD6, 0x00),       // electric yellow
            error: egui::Color32::from_rgb(0xFF, 0x22, 0x55),         // neon red

            // Spectrum: neon cyan primary, hot pink phase
            spectrum_line: egui::Color32::from_rgb(0x00, 0xE5, 0xFF),   // electric cyan
            spectrum_phase: egui::Color32::from_rgb(0x00, 0xFF, 0x88),  // neon green
            spectrum_imaginary: egui::Color32::from_rgb(0xFF, 0x00, 0x8C), // hot pink
            peak_marker: egui::Color32::from_rgb(0xFF, 0xD6, 0x00),     // electric yellow
            peak_label: egui::Color32::from_rgb(0xFF, 0xC0, 0x00),
            multiplet_label: egui::Color32::from_rgb(0xBD, 0x00, 0xFF), // neon purple
            integration_colors: [
                egui::Color32::from_rgba_premultiplied(0x00, 0xFF, 0xE0, 0x40), // cyan
                egui::Color32::from_rgba_premultiplied(0xFF, 0x00, 0x8C, 0x40), // pink
                egui::Color32::from_rgba_premultiplied(0xBD, 0x00, 0xFF, 0x40), // purple
                egui::Color32::from_rgba_premultiplied(0xFF, 0xD6, 0x00, 0x40), // yellow
            ],
            j_coupling_color: egui::Color32::from_rgb(0xFF, 0x8C, 0x00), // orange neon
            baseline_marker: egui::Color32::from_rgb(0x8B, 0x5C, 0xF6),

            // Tabs: neon pink active
            tab_active_bg: egui::Color32::from_rgb(0xFF, 0x00, 0x8C),
            tab_active_text: egui::Color32::WHITE,
            tab_inactive_bg: egui::Color32::from_rgb(0x1E, 0x1C, 0x34),
            tab_inactive_text: egui::Color32::from_rgb(0x8B, 0x88, 0xA8),

            // Status bar
            status_bar_bg: egui::Color32::from_rgb(0x0A, 0x08, 0x16),
            status_text: egui::Color32::from_rgb(0xA0, 0x9E, 0xB8),

            // Mode indicator: neon glow
            mode_picking_bg: egui::Color32::from_rgba_premultiplied(0xFF, 0x00, 0x8C, 0x30),
            mode_picking_text: egui::Color32::from_rgb(0xFF, 0x80, 0xC0),

            shadow_color: egui::Color32::from_rgba_premultiplied(0xBD, 0x00, 0xFF, 0x20),

            is_dark: true,
        }
    }
}

/// Apply a theme to the egui context
pub fn apply_theme(ctx: &egui::Context, theme: AppTheme) {
    let c = ThemeColors::from_theme(theme);

    let mut visuals = if c.is_dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    visuals.panel_fill = c.panel_fill;
    visuals.window_fill = c.window_fill;
    visuals.faint_bg_color = c.faint_bg;

    visuals.widgets.noninteractive.bg_fill = c.widget_bg;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(0.5, c.widget_bg_stroke);
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(3);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, c.text_secondary);

    visuals.widgets.inactive.bg_fill = c.widget_inactive_bg;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(0.5, c.widget_inactive_stroke);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(4);

    visuals.widgets.hovered.bg_fill = c.widget_hovered_bg;
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, c.widget_hovered_stroke);

    visuals.widgets.active.bg_fill = c.widget_active_bg;
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.5, c.widget_active_fg);

    visuals.selection.bg_fill = c.selection_bg;
    visuals.selection.stroke = egui::Stroke::new(1.5, c.selection_stroke);

    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 2],
        blur: 8,
        spread: 0,
        color: c.shadow_color,
    };

    ctx.set_visuals(visuals);
}

/// Get the active cursor mode description for the spectrum view
pub fn cursor_mode_label(state: &super::spectrum_view::SpectrumViewState, phase_active: bool) -> Option<(&'static str, &'static str, egui::Color32)> {
    if phase_active {
        return Some(("⟳ PHASING", "Drag H→PH0, V→PH1", egui::Color32::from_rgb(0x00, 0xCC, 0x66)));
    }
    if state.peak_picking {
        return Some(("🎯 PEAK PICK", "Click to add · Shift+click to remove", egui::Color32::from_rgb(0xFF, 0x44, 0x44)));
    }
    if state.baseline_picking {
        return Some(("📐 BASELINE", "Click to add baseline points", egui::Color32::from_rgb(0x88, 0x88, 0xCC)));
    }
    if state.integration_picking {
        let msg = if state.integration_start.is_some() {
            "Click end point of region"
        } else {
            "Click start point of region"
        };
        return Some(("∫ INTEGRATE", msg, egui::Color32::from_rgb(0xCC, 0x44, 0xCC)));
    }
    if state.j_coupling_picking {
        let msg = if state.j_coupling_first.is_some() {
            "Click second peak"
        } else {
            "Click first peak"
        };
        return Some(("📏 J-COUPLE", msg, egui::Color32::from_rgb(0xFF, 0x88, 0x00)));
    }
    None
}
