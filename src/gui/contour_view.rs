/// 2D Contour plot viewer for 2D NMR experiments (COSY, HSQC, HMBC)
///
/// Projections are rendered as overlays INSIDE the single contour `Plot`
/// widget.  Because they share the identical coordinate transform their
/// PPM alignment with the contour is guaranteed — not patched.

use std::path::PathBuf;
use egui_plot::{HLine, Line, Plot, PlotPoints, Points, PlotUi, Polygon, Text, VLine};

use crate::data::spectrum::{AxisParams, QuadMode, SpectrumData};

/// Actions that the contour view can request from the app
#[derive(Debug, Clone, PartialEq)]
pub enum ContourAction {
    None,
    /// Request FT of the 2D data
    RequestFT,
    /// User wants to load a 1D spectrum for the F2 (top, ¹H) projection
    LoadF2Projection(PathBuf),
    /// User wants to load a 1D spectrum for the F1 (side, ¹³C) projection
    LoadF1Projection(PathBuf),
    /// Clear the F2 external projection
    ClearF2Projection,
    /// Clear the F1 external projection
    ClearF1Projection,
}

/// State for the 2D contour viewer
#[derive(Debug, Clone)]
pub struct ContourViewState {
    pub num_levels: usize,
    pub threshold: f64,
    pub positive_color: egui::Color32,
    pub negative_color: egui::Color32,
    pub show_projections: bool,
    /// External 1D spectrum for F2 (top) projection (e.g. ¹H)
    pub f2_projection_spectrum: Option<SpectrumData>,
    /// Label for F2 projection source
    pub f2_projection_label: String,
    /// External 1D spectrum for F1 (side) projection (e.g. ¹³C)
    pub f1_projection_spectrum: Option<SpectrumData>,
    /// Label for F1 projection source
    pub f1_projection_label: String,
    /// Set to true when bounds should be recalculated (e.g. new file loaded)
    pub needs_reset: bool,
}

impl Default for ContourViewState {
    fn default() -> Self {
        Self {
            num_levels: 10,
            threshold: 0.1,
            positive_color: egui::Color32::from_rgb(0x1A, 0x47, 0x80),
            negative_color: egui::Color32::from_rgb(0xB8, 0x3A, 0x3A),
            show_projections: true,
            f2_projection_spectrum: None,
            f2_projection_label: String::new(),
            f1_projection_spectrum: None,
            f1_projection_label: String::new(),
            needs_reset: true,
        }
    }
}

/// Fraction of the visible range that each projection band occupies.
const PROJ_BAND_FRAC: f64 = 0.18;

/// Compute the F2 projection (max absolute value per column) and F1 projection (per row).
///
/// F2 projection returns `[-ppm_x, intensity]` — X matches contour X, Y = normalised intensity.
/// F1 projection returns `[intensity, ppm_y]` — Y matches contour Y, X = normalised intensity.
/// Both projections are normalised to [0, 1] so they scale consistently with external projections.
fn compute_projections(spectrum: &SpectrumData) -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
    let n_rows = spectrum.data_2d.len();
    if n_rows == 0 {
        return (Vec::new(), Vec::new());
    }
    let n_cols = spectrum.data_2d[0].len();

    // F2 projection: for each column, take the max across all rows
    let mut f2_raw = Vec::with_capacity(n_cols);
    for col_idx in 0..n_cols {
        let mut max_val = 0.0f64;
        for row in &spectrum.data_2d {
            if col_idx < row.len() {
                let v = row[col_idx].abs();
                if v > max_val {
                    max_val = v;
                }
            }
        }
        let x = if !spectrum.axes.is_empty() {
            spectrum.axes[0].index_to_ppm(col_idx)
        } else {
            col_idx as f64
        };
        f2_raw.push((-x, max_val));
    }

    // F1 projection: for each row, take the max across all columns
    let mut f1_raw = Vec::with_capacity(n_rows);
    for row_idx in 0..n_rows {
        let max_val = spectrum.data_2d[row_idx]
            .iter()
            .map(|v| v.abs())
            .fold(0.0f64, f64::max);
        let y = if spectrum.axes.len() >= 2 {
            spectrum.axes[1].index_to_ppm(row_idx)
        } else {
            row_idx as f64
        };
        f1_raw.push((max_val, y));
    }

    // Normalise to [0, 1]
    let f2_max = f2_raw.iter().map(|&(_, v)| v).fold(0.0f64, f64::max);
    let f2_scale = if f2_max > 0.0 { 1.0 / f2_max } else { 1.0 };
    let f2_proj: Vec<[f64; 2]> = f2_raw.iter().map(|&(x, v)| [x, v * f2_scale]).collect();

    let f1_max = f1_raw.iter().map(|&(v, _)| v).fold(0.0f64, f64::max);
    let f1_scale = if f1_max > 0.0 { 1.0 / f1_max } else { 1.0 };
    let f1_proj: Vec<[f64; 2]> = f1_raw.iter().map(|&(v, y)| [v * f1_scale, y]).collect();

    (f2_proj, f1_proj)
}

/// Map a 1D spectrum to F2 (top) projection plot points using native resolution.
///
/// Only points whose ppm falls within the 2D axis range are kept, and
/// normalisation uses the maximum within that visible window.
fn spectrum_to_f2_projection(spec: &SpectrumData, axis_2d: &AxisParams) -> Vec<[f64; 2]> {
    if spec.real.is_empty() || spec.axes.is_empty() {
        return Vec::new();
    }
    let n = spec.real.len();

    // 2D axis ppm bounds (may be hi→lo or lo→hi)
    let a = axis_2d.index_to_ppm(0);
    let b = axis_2d.index_to_ppm(axis_2d.num_points.saturating_sub(1));
    let (ppm_lo, ppm_hi) = if a < b { (a, b) } else { (b, a) };

    // Collect only points inside the 2D window
    let clipped: Vec<(f64, f64)> = (0..n)
        .filter_map(|i| {
            let ppm = spec.axes[0].index_to_ppm(i);
            if ppm >= ppm_lo && ppm <= ppm_hi {
                Some((ppm, spec.real[i]))
            } else {
                None
            }
        })
        .collect();

    if clipped.is_empty() {
        return Vec::new();
    }

    // Normalise within the visible range
    let max_abs = clipped.iter().map(|(_, v)| v.abs()).fold(0.0f64, f64::max);
    let scale = if max_abs > 0.0 { 1.0 / max_abs } else { 1.0 };

    clipped.iter().map(|&(ppm, v)| [-ppm, v * scale]).collect()
}

/// Map a 1D spectrum to F1 (side) projection plot points using native resolution.
///
/// Only points whose ppm falls within the 2D axis range are kept, and
/// normalisation uses the maximum within that visible window.
fn spectrum_to_f1_projection(spec: &SpectrumData, axis_2d: &AxisParams) -> Vec<[f64; 2]> {
    if spec.real.is_empty() || spec.axes.is_empty() {
        return Vec::new();
    }
    let n = spec.real.len();

    let a = axis_2d.index_to_ppm(0);
    let b = axis_2d.index_to_ppm(axis_2d.num_points.saturating_sub(1));
    let (ppm_lo, ppm_hi) = if a < b { (a, b) } else { (b, a) };

    let clipped: Vec<(f64, f64)> = (0..n)
        .filter_map(|i| {
            let ppm = spec.axes[0].index_to_ppm(i);
            if ppm >= ppm_lo && ppm <= ppm_hi {
                Some((ppm, spec.real[i]))
            } else {
                None
            }
        })
        .collect();

    if clipped.is_empty() {
        return Vec::new();
    }

    let max_abs = clipped.iter().map(|(_, v)| v.abs()).fold(0.0f64, f64::max);
    let scale = if max_abs > 0.0 { 1.0 / max_abs } else { 1.0 };

    clipped.iter().map(|&(ppm, v)| [v * scale, ppm]).collect()
}

// ─── projection remapping ────────────────────────────────────────────

/// Linearly remap one component of 2-element plot points from their natural
/// range to a destination band `[dst_min, dst_max]` (with 5 % inner padding).
///
/// `field` selects which component to remap: 0 ⇒ X, 1 ⇒ Y.
fn remap_projection(
    data: &[[f64; 2]],
    field: usize,
    dst_min: f64,
    dst_max: f64,
) -> Vec<[f64; 2]> {
    if data.is_empty() {
        return Vec::new();
    }
    let src_min = data.iter().map(|p| p[field]).fold(f64::INFINITY, f64::min);
    let src_max = data.iter().map(|p| p[field]).fold(-f64::INFINITY, f64::max);
    let src_range = src_max - src_min;

    let pad = (dst_max - dst_min) * 0.05;
    let d_min = dst_min + pad;
    let d_max = dst_max - pad;

    if src_range <= 0.0 {
        // Flat projection — place everything on the baseline
        return data
            .iter()
            .map(|p| {
                let mut out = *p;
                out[field] = d_min;
                out
            })
            .collect();
    }

    let scale = (d_max - d_min) / src_range;
    data.iter()
        .map(|p| {
            let mut out = *p;
            out[field] = d_min + (p[field] - src_min) * scale;
            out
        })
        .collect()
}

// ─── main view ───────────────────────────────────────────────────────

/// Show a 2D spectrum as a scatter/contour plot with optional 1D projection
/// overlays.  Everything is rendered inside **one** `Plot` widget so that
/// the projections share the exact coordinate transform of the contour —
/// PPM alignment is structural, not patched.
///
/// Returns a `ContourAction` for the app to handle.
pub fn show_spectrum_2d(
    ui: &mut egui::Ui,
    spectrum: &mut SpectrumData,
    state: &mut ContourViewState,
) -> ContourAction {
    let mut action = ContourAction::None;

    if spectrum.data_2d.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.heading("No 2D spectrum data loaded");
        });
        return ContourAction::None;
    }

    let n_rows = spectrum.data_2d.len();
    let n_cols = if n_rows > 0 {
        spectrum.data_2d[0].len()
    } else {
        0
    };

    let has_axes = !spectrum.axes.is_empty();
    let has_y_axis = spectrum.axes.len() >= 2;

    // X/Y labels
    let x_label = if has_axes {
        format!("{} (ppm)", spectrum.axes[0].label)
    } else {
        "F2 (points)".to_string()
    };
    let y_label = if has_y_axis {
        format!("{} (ppm)", spectrum.axes[1].label)
    } else {
        "F1 (points)".to_string()
    };

    // Controls row
    ui.horizontal(|ui| {
        ui.label(format!("{} | 2D ({}×{})", spectrum.experiment_type, n_rows, n_cols));
        ui.separator();
        let needs_ft = !spectrum.is_frequency_domain || !spectrum.is_freq_f1;
        if needs_ft {
            let label = if spectrum.is_frequency_domain && !spectrum.is_freq_f1 {
                "F1 FID (needs FT)"
            } else {
                "FID (time domain)"
            };
            ui.label(
                egui::RichText::new(label)
                    .color(egui::Color32::from_rgb(0xCC, 0x88, 0x00))
                    .small(),
            );
            // Quad mode selector (shown pre-FT for all 2D experiments)
            egui::ComboBox::from_id_salt("quad_mode")
                .selected_text(format!("{}", spectrum.quad_mode))
                .width(120.0)
                .show_ui(ui, |ui| {
                    for mode in &[
                        QuadMode::States,
                        QuadMode::EchoAntiEcho,
                        QuadMode::StatesTPPI,
                        QuadMode::TPPI,
                        QuadMode::Magnitude,
                    ] {
                        ui.selectable_value(&mut spectrum.quad_mode, *mode, format!("{}", mode));
                    }
                });
            if ui.button("🔄 2D FT").clicked() {
                action = ContourAction::RequestFT;
            }
            ui.separator();
        }
        ui.add(
            egui::Slider::new(&mut state.threshold, 0.001..=1.0)
                .text("Threshold")
                .logarithmic(true)
                .fixed_decimals(3),
        );
        ui.separator();
        ui.add(
            egui::Slider::new(&mut state.num_levels, 2..=20)
                .text("Levels"),
        );
        ui.separator();
        ui.label("Pos:");
        ui.color_edit_button_srgba(&mut state.positive_color);
        ui.label("Neg:");
        ui.color_edit_button_srgba(&mut state.negative_color);
        ui.separator();
        ui.checkbox(&mut state.show_projections, "Projections");
        if has_axes {
            ui.separator();
            ui.label(
                egui::RichText::new(format!("F2: {} | F1: {}", x_label, y_label))
                    .small()
                    .color(egui::Color32::GRAY),
            );
        }
    });

    // ── Projection selection row (only when projections are visible) ──
    if state.show_projections {
        // Derive nucleus labels from the 2D spectrum axes
        let f2_nucleus = if has_axes {
            spectrum.axes[0].nucleus.to_string()
        } else {
            "F2".to_string()
        };
        let f1_nucleus = if has_y_axis {
            spectrum.axes[1].nucleus.to_string()
        } else {
            "F1".to_string()
        };

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Projections:")
                    .small()
                    .strong(),
            );
            ui.separator();

            // F2 (top) projection
            ui.label(egui::RichText::new(format!("F2 ({}):", f2_nucleus)).small());
            if state.f2_projection_spectrum.is_some() {
                ui.label(
                    egui::RichText::new(&state.f2_projection_label)
                        .small()
                        .color(egui::Color32::from_rgb(0x40, 0xA0, 0x40)),
                );
                if ui.small_button("✕").on_hover_text("Remove F2 projection").clicked() {
                    action = ContourAction::ClearF2Projection;
                }
            } else {
                ui.label(egui::RichText::new("auto").small().color(egui::Color32::GRAY));
                if ui.small_button(format!("📂 Load {}", f2_nucleus))
                    .on_hover_text(format!("Load saved {} project for F2 projection", f2_nucleus))
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title(format!("Select saved {} project for F2 projection", f2_nucleus))
                        .add_filter("NMR Project", &["nmrproj"])
                        .pick_file()
                    {
                        action = ContourAction::LoadF2Projection(path);
                    }
                }
            }

            ui.separator();

            // F1 (side) projection
            ui.label(egui::RichText::new(format!("F1 ({}):", f1_nucleus)).small());
            if state.f1_projection_spectrum.is_some() {
                ui.label(
                    egui::RichText::new(&state.f1_projection_label)
                        .small()
                        .color(egui::Color32::from_rgb(0x40, 0xA0, 0x40)),
                );
                if ui.small_button("✕").on_hover_text("Remove F1 projection").clicked() {
                    action = ContourAction::ClearF1Projection;
                }
            } else {
                ui.label(egui::RichText::new("auto").small().color(egui::Color32::GRAY));
                if ui.small_button(format!("📂 Load {}", f1_nucleus))
                    .on_hover_text(format!("Load saved {} project for F1 projection", f1_nucleus))
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title(format!("Select saved {} project for F1 projection", f1_nucleus))
                        .add_filter("NMR Project", &["nmrproj"])
                        .pick_file()
                    {
                        action = ContourAction::LoadF1Projection(path);
                    }
                }
            }
        });
    }

    // Find the maximum value for normalization
    let max_val = spectrum
        .data_2d
        .iter()
        .flat_map(|row| row.iter())
        .map(|v| v.abs())
        .fold(0.0f64, f64::max);

    if max_val == 0.0 {
        ui.label("All zero data");
        return ContourAction::None;
    }

    let threshold_abs = state.threshold * max_val;

    // X negated so high ppm is on the LEFT (NMR convention).
    // Y negated so high ppm is at the BOTTOM and axis reads low→high
    // going up (matching the X-axis direction convention).
    let x_sign: f64 = -1.0;
    let y_sign: f64 = -1.0;

    // Collect points for multiple levels (BUG 2)
    let num_levels = state.num_levels.max(1);
    let mut pos_levels: Vec<Vec<[f64; 2]>> = vec![Vec::new(); num_levels];
    let mut neg_levels: Vec<Vec<[f64; 2]>> = vec![Vec::new(); num_levels];

    let factor = if num_levels > 1 {
        (1.0 / state.threshold).powf(1.0 / (num_levels - 1) as f64)
    } else {
        1.0
    };
    let ln_factor = factor.ln();

    for row_idx in 0..n_rows {
        for col_idx in 0..n_cols {
            let val = spectrum.data_2d[row_idx][col_idx];
            let abs_val = val.abs();
            if abs_val >= threshold_abs {
                let x = if has_axes {
                    spectrum.axes[0].index_to_ppm(col_idx)
                } else {
                    col_idx as f64
                };
                let y = if has_y_axis {
                    spectrum.axes[1].index_to_ppm(row_idx)
                } else {
                    row_idx as f64
                };
                let p = [x_sign * x, y_sign * y];

                let max_level = if num_levels > 1 && abs_val > threshold_abs {
                    ((abs_val / threshold_abs).ln() / ln_factor).floor() as usize
                } else {
                    0
                };
                let max_level = max_level.min(num_levels - 1);

                if val > 0.0 {
                    for i in 0..=max_level {
                        pos_levels[i].push(p);
                    }
                } else {
                    for i in 0..=max_level {
                        neg_levels[i].push(p);
                    }
                }
            }
        }
    }

    // ── Projection data (computed before plot, used inside closure) ──
    // The helpers produce data with hardcoded -ppm (for F2) or ppm (for F1).
    let (f2_proj, f1_proj) = if state.show_projections {
        let mut f2 = if let Some(ref ext) = state.f2_projection_spectrum {
            if has_axes {
                spectrum_to_f2_projection(ext, &spectrum.axes[0])
            } else {
                Vec::new()
            }
        } else {
            compute_projections(spectrum).0
        };
        // F2 projection X values are -ppm; convert to x_sign * ppm
        for p in &mut f2 {
            p[0] = -p[0] * x_sign;   // undo neg, apply sign
        }

        let mut f1 = if let Some(ref ext) = state.f1_projection_spectrum {
            if has_y_axis {
                spectrum_to_f1_projection(ext, &spectrum.axes[1])
            } else {
                Vec::new()
            }
        } else {
            compute_projections(spectrum).1
        };
        // F1 projection Y values are ppm; apply y_sign (BUG 1 fixed: removed negation)
        for p in &mut f1 {
            p[1] = p[1] * y_sign;
        }

        (f2, f1)
    } else {
        (Vec::new(), Vec::new())
    };

    // ── Axis formatters ──────────────────────────────────────────────
    // Undo the sign multiplier so labels always show positive ppm.
    let x_fmt = move |val: egui_plot::GridMark, _range: &std::ops::RangeInclusive<f64>| {
        format!("{:.1}", val.value / x_sign)
    };
    let y_fmt = move |val: egui_plot::GridMark, _range: &std::ops::RangeInclusive<f64>| {
        format!("{:.1}", val.value / y_sign)
    };

    let pos_col = state.positive_color;
    let neg_col = state.negative_color;
    let show_proj = state.show_projections;
    let draw_f1 = has_y_axis && state.show_projections;

    // Band background: pick up the current theme background so the
    // overlay looks natural in both light and dark modes.
    let panel_bg = ui.visuals().panel_fill;
    let band_bg = egui::Color32::from_rgba_unmultiplied(
        panel_bg.r(),
        panel_bg.g(),
        panel_bg.b(),
        100, // BUG 6 fixed: reduced opacity from 220
    );
    let sep_color = if ui.visuals().dark_mode {
        egui::Color32::from_rgb(120, 120, 140)
    } else {
        egui::Color32::from_rgb(100, 100, 120)
    };
    let proj_line_color = egui::Color32::from_rgb(0x40, 0x80, 0xC0);

    // ── Initial data bounds ──────────────────────────────────────────
    let (x_min_plot, x_max_plot, y_min_plot, y_max_plot) = if has_axes {
        let ax0 = &spectrum.axes[0];
        let x_hi = ax0.index_to_ppm(0);
        let x_lo = ax0.index_to_ppm(ax0.num_points.saturating_sub(1));
        let (y_lo, y_hi) = if has_y_axis {
            let ax1 = &spectrum.axes[1];
            (
                ax1.index_to_ppm(ax1.num_points.saturating_sub(1)),
                ax1.index_to_ppm(0),
            )
        } else {
            (0.0, n_rows as f64)
        };
        // Apply sign multipliers and sort so min < max
        let (xa, xb) = (x_sign * x_lo, x_sign * x_hi);
        let (ya, yb) = (y_sign * y_lo, y_sign * y_hi);
        (xa.min(xb), xa.max(xb), ya.min(yb), ya.max(yb))
    } else {
        (0.0, n_cols as f64, 0.0, n_rows as f64)
    };

    // ── Single Plot widget ───────────────────────────────────────────
    //
    // Everything — contour data AND projection overlays — lives in the
    // same Plot.  This means they share the identical coordinate
    // transform, so PPM alignment is guaranteed by construction.
    // Drag, zoom, scroll operate on the whole view uniformly.
    let available_h = ui.available_height() - 4.0;

    let mut plot = Plot::new("spectrum_2d")
        .height(available_h)
        .x_axis_label(&x_label)
        .y_axis_label(&y_label)
        .allow_drag(true)
        .allow_zoom(true)
        .allow_scroll(false)
        .allow_boxed_zoom(true)
        .show_grid([true, true])
        .auto_bounds(egui::Vec2b::new(false, false))
        .set_margin_fraction(egui::Vec2::new(0.02, 0.02));

    // Only set bounds via include_x/y when reset is requested (e.g. new file load)
    if state.needs_reset {
        plot = plot
            .reset()
            .include_x(x_min_plot)
            .include_x(x_max_plot)
            .include_y(y_min_plot)
            .include_y(y_max_plot);
        state.needs_reset = false;
    }

    if has_axes {
        plot = plot.x_axis_formatter(x_fmt);
        if has_y_axis {
            plot = plot.y_axis_formatter(y_fmt);
        }
        // Fix hover tooltip to show corrected PPM values (not raw negative coords)
        let xs = x_sign;
        let ys = y_sign;
        plot = plot.label_formatter(move |_name, val| {
            format!("{:.3}, {:.3} ppm", val.x / xs, val.y / ys)
        });
    }

    plot.show(ui, |plot_ui: &mut PlotUi| {
        // ── 1) Contour scatter (Multiple levels - BUG 2) ─────────────
        for i in 0..num_levels {
            let alpha = 80 + (175 * i / num_levels) as u8;
            let p_col = pos_col.linear_multiply(alpha as f32 / 255.0);
            let n_col = neg_col.linear_multiply(alpha as f32 / 255.0);

            if !pos_levels[i].is_empty() {
                plot_ui.points(
                    Points::new(PlotPoints::from(pos_levels[i].clone()))
                        .name(format!("Pos Level {}", i))
                        .color(p_col)
                        .radius(1.5),
                );
            }
            if !neg_levels[i].is_empty() {
                plot_ui.points(
                    Points::new(PlotPoints::from(neg_levels[i].clone()))
                        .name(format!("Neg Level {}", i))
                        .color(n_col)
                        .radius(1.5),
                );
            }
        }

        // ── 2) Crosshairs (BUG 4) ────────────────────────────────────
        if let Some(pos) = plot_ui.pointer_coordinate() {
            let x_ppm = pos.x / x_sign;
            let y_ppm = pos.y / y_sign;
            
            plot_ui.vline(VLine::new(pos.x).color(egui::Color32::GRAY.linear_multiply(0.5)));
            plot_ui.hline(HLine::new(pos.y).color(egui::Color32::GRAY.linear_multiply(0.5)));
            
            let text = format!("{:.3}, {:.3} ppm", x_ppm, y_ppm);
            plot_ui.text(
                Text::new(pos, text)
                    .anchor(egui::Align2::LEFT_BOTTOM)
                    .color(egui::Color32::GRAY)
            );
        }

        // ── 3) Projection overlays (drawn ON TOP of contour) ────────
        //
        // Each projection occupies a fixed fraction of the current
        // visible range (PROJ_BAND_FRAC ≈ 18 %).  A semi-transparent
        // background polygon masks the contour beneath, a separator
        // line marks the boundary, and the projection trace is drawn
        // on top.  Because the trace X/Y coordinates come from the
        // same ppm axis as the contour, alignment is exact.
        if show_proj {
            let bounds = plot_ui.plot_bounds();
            let bmin = bounds.min();
            let bmax = bounds.max();
            let bw = bmax[0] - bmin[0];
            let bh = bmax[1] - bmin[1];

            if bw > 0.0 && bh > 0.0 {
                // ── F2 (top) projection ──────────────────────────────
                let f2_base = bmax[1] - bh * PROJ_BAND_FRAC;
                let f2_top = bmax[1];

                // Semi-transparent background to mask underlying points
                plot_ui.polygon(
                    Polygon::new(PlotPoints::from(vec![
                        [bmin[0], f2_base],
                        [bmax[0], f2_base],
                        [bmax[0], f2_top],
                        [bmin[0], f2_top],
                    ]))
                    .fill_color(band_bg)
                    .width(0.0)
                    .allow_hover(false)
                    .name(""),
                );

                // Separator line
                plot_ui.hline(
                    HLine::new(f2_base)
                        .color(sep_color)
                        .width(1.0)
                        .allow_hover(false)
                        .name(""),
                );

                // F2 projection trace — remap Y (intensity) into band
                if !f2_proj.is_empty() {
                    let mapped = remap_projection(&f2_proj, 1, f2_base, f2_top);
                    plot_ui.line(
                        Line::new(PlotPoints::from(mapped))
                            .color(proj_line_color)
                            .width(1.2)
                            .allow_hover(false)
                            .name("F2 projection"),
                    );
                }

                // ── F1 (right) projection ────────────────────────────
                if draw_f1 {
                    let f1_left = bmax[0] - bw * PROJ_BAND_FRAC;
                    let f1_right = bmax[0];

                    // Background
                    plot_ui.polygon(
                        Polygon::new(PlotPoints::from(vec![
                            [f1_left, bmin[1]],
                            [f1_right, bmin[1]],
                            [f1_right, bmax[1]],
                            [f1_left, bmax[1]],
                        ]))
                        .fill_color(band_bg)
                        .width(0.0)
                        .allow_hover(false)
                        .name(""),
                    );

                    // Separator
                    plot_ui.vline(
                        VLine::new(f1_left)
                            .color(sep_color)
                            .width(1.0)
                            .allow_hover(false)
                            .name(""),
                    );

                    // F1 projection trace — remap X (intensity) into band
                    if !f1_proj.is_empty() {
                        let mapped = remap_projection(&f1_proj, 0, f1_left, f1_right);
                        plot_ui.line(
                            Line::new(PlotPoints::from(mapped))
                                .color(proj_line_color)
                                .width(1.2)
                                .allow_hover(false)
                                .name("F1 projection"),
                        );
                    }
                }
            }
        }
    });

    action
}
