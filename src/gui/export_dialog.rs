/// Export settings dialog — configure image export options

/// Export settings
#[derive(Debug, Clone)]
pub struct ExportSettings {
    /// PPM range: left (high ppm)
    pub ppm_start: f64,
    /// PPM range: right (low ppm)
    pub ppm_end: f64,
    /// Use custom ppm range (false = auto from data)
    pub use_custom_range: bool,
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// Include peak labels
    pub show_peaks: bool,
    /// Include integration regions
    pub show_integrations: bool,
    /// Include multiplet labels
    pub show_multiplets: bool,
    /// Custom title (empty = auto from spectrum metadata)
    pub custom_title: String,
    /// Use custom title
    pub use_custom_title: bool,
    /// Line width for spectrum trace
    pub line_width: f32,
    /// Show grid lines
    pub show_grid: bool,
    /// Export format: 0 = PNG, 1 = SVG
    pub format: usize,
    /// Y-axis: clip negatives (for 1H/13C)
    pub clip_negatives: bool,
    /// DPI for print-quality output
    pub dpi: u32,
    /// Scale factor for peak triangle markers (1.0 = default)
    pub marker_scale: f32,
    /// Scale factor for all text elements (1.0 = default)
    pub font_scale: f32,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            ppm_start: 14.0,
            ppm_end: -1.0,
            use_custom_range: false,
            width: 2400,
            height: 1800,
            show_peaks: true,
            show_integrations: true,
            show_multiplets: true,
            custom_title: String::new(),
            use_custom_title: false,
            line_width: 1.5,
            show_grid: true,
            format: 0, // PNG
            clip_negatives: false,
            dpi: 300,
            marker_scale: 1.0,
            font_scale: 1.0,
        }
    }
}

/// Dialog state
#[derive(Debug, Clone)]
pub struct ExportDialogState {
    pub open: bool,
    pub settings: ExportSettings,
    pub pending_path: Option<std::path::PathBuf>,
}

impl Default for ExportDialogState {
    fn default() -> Self {
        Self {
            open: false,
            settings: ExportSettings::default(),
            pending_path: None,
        }
    }
}

/// Action from the export dialog
#[derive(Debug, Clone, PartialEq)]
pub enum ExportAction {
    None,
    Export,
    Cancel,
}

/// Show the export dialog. Returns the action taken.
pub fn show_export_dialog(
    ctx: &egui::Context,
    state: &mut ExportDialogState,
    has_peaks: bool,
    has_integrations: bool,
    has_multiplets: bool,
) -> ExportAction {
    if !state.open {
        return ExportAction::None;
    }

    let mut action = ExportAction::None;

    egui::Window::new("🖼 Export Image Settings")
        .open(&mut state.open)
        .collapsible(false)
        .resizable(false)
        .default_width(400.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.heading("Image Export Configuration");
            ui.add_space(8.0);

            // ── PPM Range ──
            ui.group(|ui| {
                ui.label("📏 PPM Range");
                ui.checkbox(
                    &mut state.settings.use_custom_range,
                    "Use custom PPM range",
                );
                if state.settings.use_custom_range {
                    ui.horizontal(|ui| {
                        ui.label("From:");
                        ui.add(
                            egui::DragValue::new(&mut state.settings.ppm_start)
                                .speed(0.1)
                                .range(-50.0..=300.0)
                                .suffix(" ppm"),
                        );
                        ui.label("To:");
                        ui.add(
                            egui::DragValue::new(&mut state.settings.ppm_end)
                                .speed(0.1)
                                .range(-50.0..=300.0)
                                .suffix(" ppm"),
                        );
                    });
                } else {
                    ui.label("Auto range from data");
                }
            });

            ui.add_space(4.0);

            // ── Dimensions ──
            ui.group(|ui| {
                ui.label("📐 Image Dimensions");
                ui.horizontal(|ui| {
                    ui.label("Width:");
                    ui.add(
                        egui::DragValue::new(&mut state.settings.width)
                            .speed(10)
                            .range(800..=8000)
                            .suffix(" px"),
                    );
                    ui.label("Height:");
                    ui.add(
                        egui::DragValue::new(&mut state.settings.height)
                            .speed(10)
                            .range(400..=4000)
                            .suffix(" px"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("DPI:");
                    ui.add(
                        egui::DragValue::new(&mut state.settings.dpi)
                            .speed(10)
                            .range(72..=1200),
                    );
                    // Presets
                    if ui.button("Screen (150)").clicked() {
                        state.settings.dpi = 150;
                        state.settings.width = 1600;
                        state.settings.height = 500;
                    }
                    if ui.button("Print (300)").clicked() {
                        state.settings.dpi = 300;
                        state.settings.width = 3200;
                        state.settings.height = 1000;
                    }
                    if ui.button("HiRes (600)").clicked() {
                        state.settings.dpi = 600;
                        state.settings.width = 6400;
                        state.settings.height = 2000;
                    }
                });
            });

            ui.add_space(4.0);

            // ── Content ──
            ui.group(|ui| {
                ui.label("📋 Content");
                if has_peaks {
                    ui.checkbox(&mut state.settings.show_peaks, "Show peak labels");
                }
                if has_integrations {
                    ui.checkbox(
                        &mut state.settings.show_integrations,
                        "Show integration regions",
                    );
                }
                if has_multiplets {
                    ui.checkbox(
                        &mut state.settings.show_multiplets,
                        "Show multiplet labels",
                    );
                }
                ui.checkbox(&mut state.settings.show_grid, "Show grid lines");
                ui.checkbox(
                    &mut state.settings.clip_negatives,
                    "Clip negative intensities",
                );
            });

            ui.add_space(4.0);

            // ── Title ──
            ui.group(|ui| {
                ui.label("📝 Title");
                ui.checkbox(&mut state.settings.use_custom_title, "Custom title");
                if state.settings.use_custom_title {
                    ui.text_edit_singleline(&mut state.settings.custom_title);
                } else {
                    ui.label("Auto-generated from spectrum metadata");
                }
            });

            ui.add_space(4.0);

            // ── Style ──
            ui.group(|ui| {
                ui.label("🎨 Style");
                ui.add(
                    egui::Slider::new(&mut state.settings.line_width, 0.5..=4.0)
                        .text("Line width")
                        .fixed_decimals(1),
                );
            });

            ui.add_space(4.0);

            // ── Format ──
            ui.group(|ui| {
                ui.label("💾 Format");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut state.settings.format, 0, "PNG");
                    ui.selectable_value(&mut state.settings.format, 1, "SVG");
                });
            });

            ui.add_space(12.0);

            // ── Buttons ──
            ui.horizontal(|ui| {
                if ui.button("📥 Export").clicked() {
                    action = ExportAction::Export;
                }
                if ui.button("Cancel").clicked() {
                    action = ExportAction::Cancel;
                }
            });
        });

    action
}
