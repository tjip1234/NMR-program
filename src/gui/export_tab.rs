/// Export tab — dedicated tab for configuring and previewing image & data exports
///
/// Replaces the modal export dialog with an inline tab that shows a live
/// preview of the spectrum as it will appear in the exported image, alongside
/// all image- and data-export settings.

use crate::data::spectrum::SpectrumData;
use crate::gui::spectrum_view::SpectrumViewState;

// ── Public types ───────────────────────────────────────────────────

/// Settings for image export (PNG / SVG)
#[derive(Debug, Clone)]
pub struct ImageExportSettings {
    pub ppm_start: f64,
    pub ppm_end: f64,
    pub use_custom_range: bool,
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
    pub show_peaks: bool,
    pub show_integrations: bool,
    pub show_multiplets: bool,
    pub show_grid: bool,
    pub clip_negatives: bool,
    pub custom_title: String,
    pub use_custom_title: bool,
    pub line_width: f32,
    /// Scale factor for peak triangle markers (1.0 = default)
    pub marker_scale: f32,
    /// Scale factor for all text elements (1.0 = default)
    pub font_scale: f32,
    /// 0 = PNG, 1 = SVG
    pub format: usize,
}

impl Default for ImageExportSettings {
    fn default() -> Self {
        Self {
            ppm_start: 14.0,
            ppm_end: -1.0,
            use_custom_range: false,
            width: 2400,
            height: 1800,
            dpi: 300,
            show_peaks: true,
            show_integrations: true,
            show_multiplets: true,
            show_grid: false,
            clip_negatives: false,
            custom_title: String::new(),
            use_custom_title: false,
            line_width: 1.5,
            marker_scale: 1.0,
            font_scale: 1.0,
            format: 0,
        }
    }
}

/// Settings for data export (CSV / TSV / TXT)
#[derive(Debug, Clone)]
pub struct DataExportSettings {
    /// 0 = CSV, 1 = TSV, 2 = TXT
    pub format: usize,
    pub include_peaks: bool,
    pub include_integrations: bool,
    pub include_multiplets: bool,
    pub include_j_couplings: bool,
    pub ppm_decimals: usize,
    pub include_header: bool,
}

impl Default for DataExportSettings {
    fn default() -> Self {
        Self {
            format: 0,
            include_peaks: true,
            include_integrations: true,
            include_multiplets: true,
            include_j_couplings: true,
            ppm_decimals: 4,
            include_header: true,
        }
    }
}

/// Persistent state for the export tab
#[derive(Debug, Clone)]
pub struct ExportTabState {
    pub image_settings: ImageExportSettings,
    pub data_settings: DataExportSettings,
    /// Which sub-section is expanded: 0 = Image, 1 = Data
    pub active_section: usize,
    /// Preview generation counter — bumped when settings change
    pub preview_gen: u32,
}

impl Default for ExportTabState {
    fn default() -> Self {
        Self {
            image_settings: ImageExportSettings::default(),
            data_settings: DataExportSettings::default(),
            active_section: 0,
            preview_gen: 0,
        }
    }
}

/// Actions the export tab can emit back to the app
#[derive(Debug, Clone, PartialEq)]
pub enum ExportTabAction {
    None,
    ExportImage,
    ExportData,
    ExportLog,
}

// ── Main UI ────────────────────────────────────────────────────────

/// Render the export tab.  Returns an action if the user clicks an export button.
pub fn show_export_tab(
    ui: &mut egui::Ui,
    state: &mut ExportTabState,
    spectrum: &SpectrumData,
    view_state: &SpectrumViewState,
) -> ExportTabAction {
    let mut action = ExportTabAction::None;

    // Horizontal split: left = settings panel, right = live preview
    let total_width = ui.available_width();
    let settings_width = (total_width * 0.32).clamp(260.0, 400.0);

    ui.horizontal_top(|ui| {
        // ── Left: settings ────────────────────────────────────
        ui.allocate_ui_with_layout(
            egui::vec2(settings_width, ui.available_height()),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(4.0);

                        // Section selector: Image | Data
                        ui.horizontal(|ui| {
                            let img_active = state.active_section == 0;
                            let img_label = egui::RichText::new("🖼 Image")
                                .size(13.0)
                                .color(if img_active {
                                    egui::Color32::WHITE
                                } else {
                                    egui::Color32::from_rgb(0x55, 0x58, 0x62)
                                });
                            let img_btn = egui::Button::new(img_label)
                                .fill(if img_active {
                                    egui::Color32::from_rgb(0x3B, 0x7D, 0xC0)
                                } else {
                                    egui::Color32::from_rgb(0xE8, 0xEA, 0xED)
                                })
                                .corner_radius(5.0);
                            if ui.add(img_btn).clicked() {
                                state.active_section = 0;
                            }

                            let data_active = state.active_section == 1;
                            let data_label = egui::RichText::new("📊 Data")
                                .size(13.0)
                                .color(if data_active {
                                    egui::Color32::WHITE
                                } else {
                                    egui::Color32::from_rgb(0x55, 0x58, 0x62)
                                });
                            let data_btn = egui::Button::new(data_label)
                                .fill(if data_active {
                                    egui::Color32::from_rgb(0x3B, 0x7D, 0xC0)
                                } else {
                                    egui::Color32::from_rgb(0xE8, 0xEA, 0xED)
                                })
                                .corner_radius(5.0);
                            if ui.add(data_btn).clicked() {
                                state.active_section = 1;
                            }
                        });

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(4.0);

                        match state.active_section {
                            0 => {
                                action = show_image_settings(ui, &mut state.image_settings, view_state);
                            }
                            1 => {
                                action = show_data_settings(ui, &mut state.data_settings, view_state);
                            }
                            _ => {}
                        }
                    });
            },
        );

        ui.separator();

        // ── Right: live preview ───────────────────────────────
        ui.vertical(|ui| {
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new("Preview")
                    .size(12.0)
                    .color(egui::Color32::from_rgb(0x88, 0x8C, 0x94)),
            );
            ui.add_space(2.0);

            match state.active_section {
                0 => show_image_preview(ui, spectrum, view_state, &state.image_settings),
                1 => show_data_preview(ui, spectrum, view_state, &state.data_settings),
                _ => {}
            }
        });
    });

    action
}

// ── Image export settings panel ───────────────────────────────────

fn show_image_settings(
    ui: &mut egui::Ui,
    s: &mut ImageExportSettings,
    view_state: &SpectrumViewState,
) -> ExportTabAction {
    let mut action = ExportTabAction::None;

    // PPM range
    ui.label(
        egui::RichText::new("PPM Range")
            .size(12.5)
            .strong()
            .color(egui::Color32::from_rgb(0x2A, 0x2E, 0x36)),
    );
    ui.checkbox(&mut s.use_custom_range, "Custom range");
    if s.use_custom_range {
        ui.horizontal(|ui| {
            ui.label("From");
            ui.add(
                egui::DragValue::new(&mut s.ppm_start)
                    .speed(0.1)
                    .suffix(" ppm"),
            );
            ui.label("to");
            ui.add(
                egui::DragValue::new(&mut s.ppm_end)
                    .speed(0.1)
                    .suffix(" ppm"),
            );
        });
    }
    ui.add_space(6.0);

    // Dimensions
    ui.label(
        egui::RichText::new("Dimensions")
            .size(12.5)
            .strong()
            .color(egui::Color32::from_rgb(0x2A, 0x2E, 0x36)),
    );
    ui.horizontal(|ui| {
        ui.add(
            egui::DragValue::new(&mut s.width)
                .speed(10)
                .range(800..=8000)
                .prefix("W ")
                .suffix(" px"),
        );
        ui.label("×");
        ui.add(
            egui::DragValue::new(&mut s.height)
                .speed(10)
                .range(400..=4000)
                .prefix("H ")
                .suffix(" px"),
        );
    });
    ui.horizontal(|ui| {
        ui.label("DPI");
        ui.add(
            egui::DragValue::new(&mut s.dpi)
                .speed(10)
                .range(72..=1200),
        );
    });
    ui.horizontal(|ui| {
        if ui.small_button("Screen").clicked() {
            s.dpi = 150;
            s.width = 1200;
            s.height = 900;
        }
        if ui.small_button("Print").clicked() {
            s.dpi = 300;
            s.width = 2400;
            s.height = 1800;
        }
        if ui.small_button("HiRes").clicked() {
            s.dpi = 600;
            s.width = 4800;
            s.height = 3600;
        }
    });
    ui.add_space(6.0);

    // Content toggles
    ui.label(
        egui::RichText::new("Content")
            .size(12.5)
            .strong()
            .color(egui::Color32::from_rgb(0x2A, 0x2E, 0x36)),
    );
    if !view_state.peaks.is_empty() {
        ui.checkbox(&mut s.show_peaks, "Peak labels");
    }
    if !view_state.integrations.is_empty() {
        ui.checkbox(&mut s.show_integrations, "Integrations");
    }
    if !view_state.multiplets.is_empty() {
        ui.checkbox(&mut s.show_multiplets, "Multiplets");
    }
    ui.checkbox(&mut s.show_grid, "Grid lines");
    ui.checkbox(&mut s.clip_negatives, "Clip negative intensities");
    ui.add_space(6.0);

    // Title
    ui.label(
        egui::RichText::new("Title")
            .size(12.5)
            .strong()
            .color(egui::Color32::from_rgb(0x2A, 0x2E, 0x36)),
    );
    ui.checkbox(&mut s.use_custom_title, "Custom title");
    if s.use_custom_title {
        ui.add(
            egui::TextEdit::singleline(&mut s.custom_title)
                .desired_width(settings_panel_width())
                .hint_text("Enter title…"),
        );
    }
    ui.add_space(6.0);

    // Style
    ui.label(
        egui::RichText::new("Style")
            .size(12.5)
            .strong()
            .color(egui::Color32::from_rgb(0x2A, 0x2E, 0x36)),
    );
    ui.add(
        egui::Slider::new(&mut s.line_width, 0.5..=4.0)
            .text("Line width")
            .fixed_decimals(1),
    );
    ui.add(
        egui::Slider::new(&mut s.marker_scale, 0.5..=3.0)
            .text("Marker scale")
            .fixed_decimals(1),
    );
    ui.add(
        egui::Slider::new(&mut s.font_scale, 0.5..=3.0)
            .text("Font scale")
            .fixed_decimals(1),
    );
    ui.add_space(6.0);

    // Format
    ui.label(
        egui::RichText::new("Format")
            .size(12.5)
            .strong()
            .color(egui::Color32::from_rgb(0x2A, 0x2E, 0x36)),
    );
    ui.horizontal(|ui| {
        ui.selectable_value(&mut s.format, 0, "PNG");
        ui.selectable_value(&mut s.format, 1, "SVG");
    });

    ui.add_space(16.0);
    if ui
        .add(
            egui::Button::new(
                egui::RichText::new("📥  Export Image…")
                    .size(14.0)
                    .color(egui::Color32::WHITE),
            )
            .fill(egui::Color32::from_rgb(0x3B, 0x7D, 0xC0))
            .corner_radius(6.0)
            .min_size(egui::vec2(200.0, 32.0)),
        )
        .clicked()
    {
        action = ExportTabAction::ExportImage;
    }

    action
}

// ── Data export settings panel ────────────────────────────────────

fn show_data_settings(
    ui: &mut egui::Ui,
    s: &mut DataExportSettings,
    view_state: &SpectrumViewState,
) -> ExportTabAction {
    let mut action = ExportTabAction::None;

    ui.label(
        egui::RichText::new("Format")
            .size(12.5)
            .strong()
            .color(egui::Color32::from_rgb(0x2A, 0x2E, 0x36)),
    );
    ui.horizontal(|ui| {
        ui.selectable_value(&mut s.format, 0, "CSV");
        ui.selectable_value(&mut s.format, 1, "TSV");
        ui.selectable_value(&mut s.format, 2, "TXT");
    });
    ui.add_space(6.0);

    ui.label(
        egui::RichText::new("Precision")
            .size(12.5)
            .strong()
            .color(egui::Color32::from_rgb(0x2A, 0x2E, 0x36)),
    );
    ui.add(
        egui::Slider::new(&mut s.ppm_decimals, 2..=6)
            .text("PPM decimals"),
    );
    ui.add_space(6.0);

    ui.label(
        egui::RichText::new("Sections")
            .size(12.5)
            .strong()
            .color(egui::Color32::from_rgb(0x2A, 0x2E, 0x36)),
    );
    let n_peaks = view_state.peaks.len();
    let n_int = view_state.integrations.len();
    let n_mult = view_state.multiplets.len();
    let n_j = view_state.j_couplings.len();

    ui.checkbox(
        &mut s.include_peaks,
        format!("Peak list ({} peaks)", n_peaks),
    );
    ui.checkbox(
        &mut s.include_integrations,
        format!("Integrations ({} regions)", n_int),
    );
    ui.checkbox(
        &mut s.include_multiplets,
        format!("Multiplets ({} found)", n_mult),
    );
    ui.checkbox(
        &mut s.include_j_couplings,
        format!("J-couplings ({} measured)", n_j),
    );
    ui.add_space(4.0);
    ui.checkbox(&mut s.include_header, "Include header / metadata");

    ui.add_space(16.0);
    if ui
        .add(
            egui::Button::new(
                egui::RichText::new("📥  Export Data…")
                    .size(14.0)
                    .color(egui::Color32::WHITE),
            )
            .fill(egui::Color32::from_rgb(0x3B, 0x7D, 0xC0))
            .corner_radius(6.0)
            .min_size(egui::vec2(200.0, 32.0)),
        )
        .clicked()
    {
        action = ExportTabAction::ExportData;
    }

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    // Log export shortcut
    if ui
        .add(
            egui::Button::new(
                egui::RichText::new("📋  Export Processing Log…")
                    .size(13.0),
            )
            .corner_radius(5.0)
            .min_size(egui::vec2(200.0, 28.0)),
        )
        .clicked()
    {
        action = ExportTabAction::ExportLog;
    }

    action
}

// ── Image preview (Painter-based, matches export layout) ──────────

fn show_image_preview(
    ui: &mut egui::Ui,
    spectrum: &SpectrumData,
    view_state: &SpectrumViewState,
    settings: &ImageExportSettings,
) {
    // Show notice for 2D data
    if spectrum.is_2d() {
        ui.label(
            egui::RichText::new("⚠ 2D spectrum — image export shows the F2 projection (1D trace)")
                .size(11.0)
                .color(egui::Color32::from_rgb(0xCC, 0x88, 0x00)),
        );
        ui.add_space(2.0);
    }

    if spectrum.real.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label("No 1D data available for export preview");
        });
        return;
    }

    // ── Data preparation (identical to export) ──
    let ppm_scale = if spectrum.is_frequency_domain && !spectrum.axes.is_empty() {
        spectrum.axes[0].ppm_scale()
    } else {
        (0..spectrum.real.len())
            .map(|i| i as f64)
            .collect::<Vec<_>>()
    };

    let (ppm_hi, ppm_lo) = if settings.use_custom_range {
        (
            settings.ppm_start.max(settings.ppm_end),
            settings.ppm_start.min(settings.ppm_end),
        )
    } else {
        let mn = ppm_scale.iter().cloned().fold(f64::INFINITY, f64::min);
        let mx = ppm_scale.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        (mx, mn)
    };
    let x_range = ppm_hi - ppm_lo;
    if x_range <= 0.0 {
        ui.centered_and_justified(|ui| {
            ui.label("Invalid PPM range");
        });
        return;
    }

    let clip = settings.clip_negatives;
    let n = spectrum.real.len().min(ppm_scale.len());

    let (y_min, y_max) = {
        let mut ymin = f64::INFINITY;
        let mut ymax = f64::NEG_INFINITY;
        for i in 0..n {
            let ppm = ppm_scale[i];
            if ppm < ppm_lo || ppm > ppm_hi {
                continue;
            }
            let v = if clip {
                spectrum.real[i].max(0.0)
            } else {
                spectrum.real[i]
            };
            if v < ymin {
                ymin = v;
            }
            if v > ymax {
                ymax = v;
            }
        }
        if clip {
            ymin = 0.0;
        }
        (ymin, ymax)
    };
    let y_range = (y_max - y_min).max(1e-12);
    let y_max_padded = y_max + y_range * 0.05;
    let y_range_padded = (y_max_padded - y_min).max(1e-12);

    if y_range_padded <= 0.0 {
        ui.centered_and_justified(|ui| {
            ui.label("No data in selected range");
        });
        return;
    }

    // ── Allocate preview rect with correct aspect ratio ──
    let aspect = settings.width as f32 / settings.height as f32;
    let avail = ui.available_size();
    let pw = avail.x.min(avail.y * aspect);
    let ph = (pw / aspect).min(avail.y);

    let (response, painter) =
        ui.allocate_painter(egui::vec2(pw, ph), egui::Sense::hover());
    let canvas = response.rect;

    // Background
    painter.rect_filled(canvas, 0.0, egui::Color32::WHITE);

    // ── Margins (same fractions as export) ──
    let ml = (pw * 0.04).max(20.0);
    let mr = (pw * 0.025).max(10.0);
    let mt = (ph * 0.08).max(15.0);
    let mb = (ph * 0.10).max(20.0);

    let plot_rect = egui::Rect::from_min_max(
        egui::pos2(canvas.left() + ml, canvas.top() + mt),
        egui::pos2(canvas.right() - mr, canvas.bottom() - mb),
    );
    if plot_rect.width() < 10.0 || plot_rect.height() < 10.0 {
        return;
    }

    // Scale from export to preview
    let scale = pw / settings.width as f32;
    let fs = settings.font_scale;
    let ms = settings.marker_scale;

    // Font sizes (scaled with minimum for readability)
    let font_sm = (10.0 * fs * scale).max(7.0);
    let font_lg = (16.0 * fs * scale).max(9.0);
    let font_md = (12.0 * fs * scale).max(8.0);
    let marker_r = (4.0 * ms * scale).max(1.5);

    // ── Coordinate helpers ──
    let ppm_to_x = |ppm: f64| -> f32 {
        plot_rect.left() + ((ppm_hi - ppm) / x_range) as f32 * plot_rect.width()
    };
    let val_to_y = |val: f64| -> f32 {
        let y_frac = 1.0 - (val - y_min) / y_range_padded;
        plot_rect.top()
            + (y_frac as f32 * plot_rect.height()).clamp(0.0, plot_rect.height())
    };

    // ── Grid ──
    if settings.show_grid {
        let grid_color = egui::Color32::from_rgb(230, 230, 235);
        let tick_step = preview_tick_step(x_range);
        let first_tick = (ppm_lo / tick_step).ceil() * tick_step;
        let mut tick = first_tick;
        while tick <= ppm_hi {
            let x = ppm_to_x(tick);
            painter.line_segment(
                [
                    egui::pos2(x, plot_rect.top()),
                    egui::pos2(x, plot_rect.bottom()),
                ],
                egui::Stroke::new(0.5, grid_color),
            );
            tick += tick_step;
        }
    }

    // ── Spectrum polyline ──
    let spec_color = egui::Color32::from_rgb(0x1A, 0x3A, 0x6B);
    let mut points: Vec<egui::Pos2> = Vec::with_capacity(n);
    for i in 0..n {
        let ppm = ppm_scale[i];
        if ppm < ppm_lo || ppm > ppm_hi {
            continue;
        }
        let x = ppm_to_x(ppm);
        let y_val = if clip {
            spectrum.real[i].max(0.0)
        } else {
            spectrum.real[i]
        };
        let y = val_to_y(y_val);
        points.push(egui::pos2(x, y));
    }
    if points.len() >= 2 {
        // Downsample for performance if needed
        let max_pts = (pw * 2.0) as usize;
        let pts = if points.len() > max_pts && max_pts > 2 {
            let step = points.len() as f64 / max_pts as f64;
            (0..max_pts)
                .map(|i| points[(i as f64 * step) as usize])
                .collect::<Vec<_>>()
        } else {
            points
        };
        painter.add(egui::Shape::line(
            pts,
            egui::Stroke::new(
                (settings.line_width * scale).max(0.5),
                spec_color,
            ),
        ));
    }

    // ── Plot border ──
    painter.rect_stroke(
        plot_rect,
        0.0,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 100, 110)),
        egui::epaint::StrokeKind::Outside,
    );

    // ── Peak markers with collision-avoidant labels ──
    if settings.show_peaks && !view_state.peaks.is_empty() {
        let peak_color = egui::Color32::from_rgb(0xD0, 0x30, 0x30);
        let leader_color = egui::Color32::from_rgb(0xC8, 0x78, 0x78);
        let font = egui::FontId::proportional(font_sm);

        struct PLabel {
            px: f32,
            py: f32,
            text: String,
            rect: egui::Rect,
            natural_y: f32,
        }

        let mut labels: Vec<PLabel> = view_state
            .peaks
            .iter()
            .filter(|p| p[0] >= ppm_lo && p[0] <= ppm_hi)
            .map(|p| {
                let px = ppm_to_x(p[0]);
                let y_val = if clip { p[1].max(0.0) } else { p[1] };
                let py = val_to_y(y_val);
                let text = format!("{:.2}", p[0]);
                let galley = painter.layout_no_wrap(text.clone(), font.clone(), peak_color);
                let tw = galley.size().x;
                let th = galley.size().y;
                let lx = px - tw / 2.0;
                let ly = py - marker_r * 3.5 - th - 2.0;
                PLabel {
                    px,
                    py,
                    text,
                    rect: egui::Rect::from_min_size(
                        egui::pos2(lx, ly),
                        egui::vec2(tw, th),
                    ),
                    natural_y: ly,
                }
            })
            .collect();

        // Collision avoidance — sort by x, shift overlapping labels up
        labels.sort_by(|a, b| {
            a.rect
                .left()
                .partial_cmp(&b.rect.left())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let pad = 3.0_f32;
        // Multi-pass collision avoidance: repeat until stable or max passes
        for _pass in 0..5 {
            let mut any_moved = false;
            for i in 0..labels.len() {
                for _iter in 0..20 {
                    let mut needs_shift = false;
                    let mut shift_to = f32::MAX;
                    let ri = labels[i].rect.expand(pad);
                    for j in 0..labels.len() {
                        if j == i {
                            continue;
                        }
                        let rj = labels[j].rect.expand(pad);
                        if ri.intersects(rj) {
                            let target = rj.top() - labels[i].rect.height() - pad * 2.0;
                            if target < shift_to {
                                shift_to = target;
                            }
                            needs_shift = true;
                        }
                    }
                    if needs_shift {
                        // Clamp so labels don't go above the title
                        let min_y = canvas.top() + font_lg + 4.0;
                        let sz = labels[i].rect.size();
                        labels[i].rect = egui::Rect::from_min_size(
                            egui::pos2(labels[i].rect.left(), shift_to.max(min_y)),
                            sz,
                        );
                        any_moved = true;
                    } else {
                        break;
                    }
                }
            }
            if !any_moved {
                break;
            }
        }

        // Draw markers, leader lines, labels
        for pl in &labels {
            // Triangle marker
            let tri_bot = pl.py - marker_r * 1.5;
            let tri_top = pl.py - marker_r * 3.5;
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(pl.px, tri_bot),
                    egui::pos2(pl.px - marker_r, tri_top),
                    egui::pos2(pl.px + marker_r, tri_top),
                ],
                peak_color,
                egui::Stroke::NONE,
            ));

            // Leader line if label was displaced
            if pl.rect.top() < pl.natural_y - 3.0 {
                painter.line_segment(
                    [
                        egui::pos2(pl.px, pl.rect.bottom() + 1.0),
                        egui::pos2(pl.px, tri_top),
                    ],
                    egui::Stroke::new(0.5, leader_color),
                );
            }

            // Label text
            painter.text(
                pl.rect.left_top(),
                egui::Align2::LEFT_TOP,
                &pl.text,
                font.clone(),
                peak_color,
            );
        }
    }

    // ── Below-plot stacked labels ──
    let tick_step = preview_tick_step(x_range);
    let tick_len = (4.0 * ms * scale).max(2.0);
    let row_gap = (3.0 * scale).max(2.0);

    // Row 1: axis tick labels
    let tick_label_y = plot_rect.bottom() + tick_len + row_gap;
    {
        let tick_font = egui::FontId::proportional(font_md);
        let tick_color = egui::Color32::from_rgb(60, 60, 70);
        let first_tick = (ppm_lo / tick_step).ceil() * tick_step;
        let mut tick = first_tick;
        while tick <= ppm_hi {
            let x = ppm_to_x(tick);
            // Tick mark
            painter.line_segment(
                [
                    egui::pos2(x, plot_rect.bottom()),
                    egui::pos2(x, plot_rect.bottom() + tick_len),
                ],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 100, 110)),
            );
            // Label
            painter.text(
                egui::pos2(x, tick_label_y),
                egui::Align2::CENTER_TOP,
                format!("{:.1}", tick),
                tick_font.clone(),
                tick_color,
            );
            tick += tick_step;
        }
    }

    // Measure tick label height
    let tick_label_h = painter
        .layout_no_wrap(
            "0.0".to_string(),
            egui::FontId::proportional(font_md),
            egui::Color32::BLACK,
        )
        .size()
        .y;
    let mut next_row_y = tick_label_y + tick_label_h + row_gap;

    // Row 2: Integration labels (if any)
    if settings.show_integrations && !view_state.integrations.is_empty() {
        let int_color = egui::Color32::from_rgb(76, 175, 80);
        let int_font = egui::FontId::proportional(font_sm);
        let first_raw = view_state
            .integrations
            .first()
            .map(|r| r.2)
            .unwrap_or(1.0)
            .abs()
            .max(1e-12);
        let ref_h = view_state.integration_reference_h;

        let mut int_label_h = 0.0f32;
        for &(start_ppm, end_ppm, raw_val) in &view_state.integrations {
            let lo = start_ppm.min(end_ppm).max(ppm_lo);
            let hi = start_ppm.max(end_ppm).min(ppm_hi);
            if lo >= hi {
                continue;
            }
            let x_lo = ppm_to_x(hi);
            let x_hi = ppm_to_x(lo);

            // Dashed boundary lines
            let dash = (4.0 * scale).max(2.0);
            let gap = (3.0 * scale).max(1.5);
            let mut y = plot_rect.top();
            while y < plot_rect.bottom() {
                let y_end = (y + dash).min(plot_rect.bottom());
                painter.line_segment(
                    [egui::pos2(x_lo, y), egui::pos2(x_lo, y_end)],
                    egui::Stroke::new(0.5, int_color),
                );
                painter.line_segment(
                    [egui::pos2(x_hi, y), egui::pos2(x_hi, y_end)],
                    egui::Stroke::new(0.5, int_color),
                );
                y += dash + gap;
            }

            // Label
            let mid_x = (x_lo + x_hi) / 2.0;
            let rel_val = raw_val / first_raw;
            let h_val = rel_val * ref_h;
            let label = format!("{:.2}H", h_val);
            let r = painter.text(
                egui::pos2(mid_x, next_row_y),
                egui::Align2::CENTER_TOP,
                label,
                int_font.clone(),
                int_color,
            );
            if r.height() > int_label_h {
                int_label_h = r.height();
            }
        }
        next_row_y += int_label_h + row_gap;
    }

    // Row 3: Multiplet labels (if any)
    if settings.show_multiplets && !view_state.multiplets.is_empty() {
        let mult_color = egui::Color32::from_rgb(0, 96, 170);
        let mult_font = egui::FontId::proportional(font_sm);

        for mult in &view_state.multiplets {
            if mult.center_ppm < ppm_lo || mult.center_ppm > ppm_hi {
                continue;
            }
            let x = ppm_to_x(mult.center_ppm);
            let label = if mult.j_hz > 0.5 {
                format!("{} J={:.1}", mult.label, mult.j_hz)
            } else {
                mult.label.clone()
            };
            painter.text(
                egui::pos2(x, next_row_y),
                egui::Align2::CENTER_TOP,
                label,
                mult_font.clone(),
                mult_color,
            );
        }
    }

    // ── Title ──
    let title = if settings.use_custom_title && !settings.custom_title.is_empty() {
        settings.custom_title.clone()
    } else {
        format!(
            "{} — {}",
            spectrum.sample_name, spectrum.experiment_type
        )
    };
    painter.text(
        egui::pos2(canvas.left() + ml, canvas.top() + 4.0),
        egui::Align2::LEFT_TOP,
        title,
        egui::FontId::proportional(font_lg),
        egui::Color32::from_rgb(40, 40, 50),
    );

    // ── X-axis title ──
    let ax_title_y = canvas.bottom() - 4.0;
    painter.text(
        egui::pos2(plot_rect.center().x, ax_title_y),
        egui::Align2::CENTER_BOTTOM,
        "Chemical Shift (ppm)",
        egui::FontId::proportional(font_md),
        egui::Color32::from_rgb(60, 60, 70),
    );

    // ── PPM range info ──
    painter.text(
        egui::pos2(canvas.right() - mr, canvas.top() + 4.0),
        egui::Align2::RIGHT_TOP,
        format!("{:.1} – {:.1} ppm", ppm_hi, ppm_lo),
        egui::FontId::proportional((font_sm - 1.0).max(6.0)),
        egui::Color32::from_rgb(120, 120, 130),
    );
}

/// Pick a nice tick step for axis labels.
fn preview_tick_step(range: f64) -> f64 {
    let nice_steps = [0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0];
    let target_ticks = 10.0;
    let raw_step = range / target_ticks;
    for &step in &nice_steps {
        if step >= raw_step {
            return step;
        }
    }
    50.0
}

// ── Data preview ──────────────────────────────────────────────────

fn show_data_preview(
    ui: &mut egui::Ui,
    spectrum: &SpectrumData,
    view_state: &SpectrumViewState,
    settings: &DataExportSettings,
) {
    // Show notice for 2D data
    if spectrum.is_2d() {
        ui.label(
            egui::RichText::new("⚠ 2D spectrum — data export shows 1D analysis results only")
                .size(11.0)
                .color(egui::Color32::from_rgb(0xCC, 0x88, 0x00)),
        );
        ui.add_space(2.0);
    }

    let sep = match settings.format {
        0 => ",",
        1 => "\t",
        _ => "\t",
    };
    let dec = settings.ppm_decimals;

    let mut preview = String::with_capacity(2048);

    // Header
    if settings.include_header {
        preview.push_str(&format!("# Sample: {}\n", spectrum.sample_name));
        preview.push_str(&format!("# Experiment: {}\n", spectrum.experiment_type));
        if !spectrum.axes.is_empty() {
            preview.push_str(&format!(
                "# Observe: {:.4} MHz  |  SW: {:.2} Hz\n",
                spectrum.axes[0].observe_freq_mhz,
                spectrum.axes[0].spectral_width_hz,
            ));
        }
        preview.push('\n');
    }

    // Peaks
    if settings.include_peaks && !view_state.peaks.is_empty() {
        let peaks = &view_state.peaks;
        let max_i = peaks
            .iter()
            .map(|p| p[1].abs())
            .fold(0.0f64, f64::max)
            .max(1e-20);
        preview.push_str(&format!("# Peak List ({} peaks)\n", peaks.len()));
        preview.push_str(&format!(
            "No{}PPM{}Intensity{}Rel%\n",
            sep, sep, sep
        ));
        for (i, p) in peaks.iter().enumerate().take(20) {
            preview.push_str(&format!(
                "{}{}{:.prec$}{}{:.4e}{}{:.1}\n",
                i + 1,
                sep,
                p[0],
                sep,
                p[1],
                sep,
                p[1] / max_i * 100.0,
                prec = dec,
            ));
        }
        if peaks.len() > 20 {
            preview.push_str(&format!("... ({} more)\n", peaks.len() - 20));
        }
        preview.push('\n');
    }

    // Integrations
    if settings.include_integrations && !view_state.integrations.is_empty() {
        let ints = &view_state.integrations;
        let first_abs = ints.first().map(|r| r.2.abs()).unwrap_or(1.0).max(1e-20);
        let ref_h = view_state.integration_reference_h;
        preview.push_str(&format!("# Integrations ({} regions, ref={:.1}H)\n", ints.len(), ref_h));
        preview.push_str(&format!("No{}Start{}End{}H_count\n", sep, sep, sep));
        for (i, &(s, e, raw)) in ints.iter().enumerate() {
            let lo = s.min(e);
            let hi = s.max(e);
            preview.push_str(&format!(
                "{}{}{:.prec$}{}{:.prec$}{}{:.2}\n",
                i + 1,
                sep,
                hi,
                sep,
                lo,
                sep,
                (raw / first_abs) * ref_h,
                prec = dec,
            ));
        }
        preview.push('\n');
    }

    // Multiplets
    if settings.include_multiplets && !view_state.multiplets.is_empty() {
        let mults = &view_state.multiplets;
        preview.push_str(&format!("# Multiplets ({} found)\n", mults.len()));
        preview.push_str(&format!("No{}Center{}Pattern{}J_Hz\n", sep, sep, sep));
        for (i, m) in mults.iter().enumerate() {
            preview.push_str(&format!(
                "{}{}{:.prec$}{}{}{}{:.2}\n",
                i + 1,
                sep,
                m.center_ppm,
                sep,
                m.label,
                sep,
                m.j_hz,
                prec = dec,
            ));
        }
        preview.push('\n');
    }

    // J-couplings
    if settings.include_j_couplings && !view_state.j_couplings.is_empty() {
        let jc = &view_state.j_couplings;
        preview.push_str(&format!("# J-Couplings ({} measured)\n", jc.len()));
        preview.push_str(&format!("No{}Peak1{}Peak2{}J_Hz\n", sep, sep, sep));
        for (i, &(p1, p2, _, j)) in jc.iter().enumerate() {
            preview.push_str(&format!(
                "{}{}{:.prec$}{}{:.prec$}{}{:.2}\n",
                i + 1,
                sep,
                p1,
                sep,
                p2,
                sep,
                j,
                prec = dec,
            ));
        }
        preview.push('\n');
    }

    if view_state.peaks.is_empty()
        && view_state.integrations.is_empty()
        && view_state.multiplets.is_empty()
        && view_state.j_couplings.is_empty()
    {
        preview.push_str("No analysis data yet.\nRun peak detection or add integrations first.\n");
    }

    // Render in a scrollable monospace area
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut preview.as_str())
                    .font(egui::TextStyle::Monospace)
                    .desired_width(ui.available_width())
                    .desired_rows(30)
                    .interactive(false),
            );
        });
}

fn settings_panel_width() -> f32 {
    220.0
}
