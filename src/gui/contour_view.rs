/// 2D Contour plot viewer for 2D NMR experiments (COSY, HSQC, HMBC)

use egui_plot::{Line, Plot, PlotPoints, Points, PlotUi};

use crate::data::spectrum::SpectrumData;

/// State for the 2D contour viewer
#[derive(Debug, Clone)]
pub struct ContourViewState {
    pub num_levels: usize,
    pub threshold: f64,
    pub positive_color: egui::Color32,
    pub negative_color: egui::Color32,
    pub show_projections: bool,
}

impl Default for ContourViewState {
    fn default() -> Self {
        Self {
            num_levels: 10,
            threshold: 0.1,
            positive_color: egui::Color32::from_rgb(0x1A, 0x47, 0x80),
            negative_color: egui::Color32::from_rgb(0xB8, 0x3A, 0x3A),
            show_projections: true,
        }
    }
}

/// Compute the F2 projection (max absolute value along each column → 1D trace along x-axis)
/// and F1 projection (max absolute value along each row → 1D trace along y-axis).
fn compute_projections(spectrum: &SpectrumData) -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
    let n_rows = spectrum.data_2d.len();
    if n_rows == 0 {
        return (Vec::new(), Vec::new());
    }
    let n_cols = spectrum.data_2d[0].len();

    // F2 projection: for each column, take the max across all rows
    let mut f2_proj = Vec::with_capacity(n_cols);
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
        f2_proj.push([-x, max_val]);
    }

    // F1 projection: for each row, take the max across all columns
    let mut f1_proj = Vec::with_capacity(n_rows);
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
        // For the side plot: x = intensity, y = ppm (rotated)
        f1_proj.push([-y, max_val]);
    }

    (f2_proj, f1_proj)
}

/// Show a 2D spectrum as a scatter/contour plot with 1D projections on axes.
/// Returns `true` if the user clicked the "2D FT" button (time-domain only).
pub fn show_spectrum_2d(
    ui: &mut egui::Ui,
    spectrum: &SpectrumData,
    state: &mut ContourViewState,
) -> bool {
    let mut request_ft = false;

    if spectrum.data_2d.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.heading("No 2D spectrum data loaded");
        });
        return false;
    }

    let n_rows = spectrum.data_2d.len();
    let n_cols = if n_rows > 0 {
        spectrum.data_2d[0].len()
    } else {
        0
    };

    // Controls row
    ui.horizontal(|ui| {
        ui.label(format!("{} | 2D ({}×{})", spectrum.experiment_type, n_rows, n_cols));
        ui.separator();
        if !spectrum.is_frequency_domain {
            ui.label(
                egui::RichText::new("FID (time domain)")
                    .color(egui::Color32::from_rgb(0xCC, 0x88, 0x00))
                    .small(),
            );
            if ui.button("🔄 2D FT").clicked() {
                request_ft = true;
            }
            ui.separator();
        }
        ui.add(
            egui::Slider::new(&mut state.threshold, 0.01..=1.0)
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
        ui.checkbox(&mut state.show_projections, "Projections");
    });

    // Find the maximum value for normalization
    let max_val = spectrum
        .data_2d
        .iter()
        .flat_map(|row| row.iter())
        .map(|v| v.abs())
        .fold(0.0f64, f64::max);

    if max_val == 0.0 {
        ui.label("All zero data");
        return false;
    }

    let threshold_abs = state.threshold * max_val;

    // Collect points above threshold
    let mut pos_points: Vec<[f64; 2]> = Vec::new();
    let mut neg_points: Vec<[f64; 2]> = Vec::new();

    for row_idx in 0..n_rows {
        for col_idx in 0..n_cols {
            let val = spectrum.data_2d[row_idx][col_idx];
            if val.abs() > threshold_abs {
                let x = if !spectrum.axes.is_empty() {
                    spectrum.axes[0].index_to_ppm(col_idx)
                } else {
                    col_idx as f64
                };
                let y = if spectrum.axes.len() >= 2 {
                    spectrum.axes[1].index_to_ppm(row_idx)
                } else {
                    row_idx as f64
                };

                if val > 0.0 {
                    pos_points.push([-x, -y]);
                } else {
                    neg_points.push([-x, -y]);
                }
            }
        }
    }

    // X/Y labels
    let x_label = if !spectrum.axes.is_empty() {
        format!("{} (ppm)", spectrum.axes[0].label)
    } else {
        "F2 (points)".to_string()
    };
    let y_label = if spectrum.axes.len() >= 2 {
        format!("{} (ppm)", spectrum.axes[1].label)
    } else {
        "F1 (points)".to_string()
    };

    // Compute projections
    let (f2_proj, f1_proj) = if state.show_projections {
        compute_projections(spectrum)
    } else {
        (Vec::new(), Vec::new())
    };

    // Axis formatters (show positive ppm even though we negate internally)
    let has_axes = !spectrum.axes.is_empty();
    let has_y_axis = spectrum.axes.len() >= 2;

    let proj_height = 100.0; // height of top projection
    let proj_width = 100.0;  // width of side projection
    let available_h = ui.available_height() - 4.0;
    let available_w = ui.available_width() - 4.0;

    if state.show_projections {
        // ── Layout: top projection + (main contour | side projection) ──
        let main_h = (available_h - proj_height - 8.0).max(100.0);
        let main_w = if has_y_axis {
            (available_w - proj_width - 8.0).max(100.0)
        } else {
            available_w
        };

        // Shared link group so all three plots pan/zoom together
        let link_id = egui::Id::new("contour_link");

        // Top: F2 projection (horizontal 1D trace) — linked on X only
        let f2_plot = Plot::new("f2_projection")
            .height(proj_height)
            .width(main_w)
            .show_axes([true, false])
            .show_grid([true, false])
            .allow_drag([true, false])
            .allow_zoom(true)
            .allow_scroll(true)
            .allow_boxed_zoom(true)
            .y_axis_label("")
            .x_axis_label("")
            .link_axis(link_id, [true, false]);

        let f2_plot = if has_axes {
            f2_plot.x_axis_formatter(|val, _range| format!("{:.1}", -val.value))
        } else {
            f2_plot
        };

        let f2_data = f2_proj.clone();
        f2_plot.show(ui, |plot_ui: &mut PlotUi| {
            if !f2_data.is_empty() {
                let line = Line::new(PlotPoints::from(f2_data))
                    .color(egui::Color32::from_rgb(0x40, 0x80, 0xC0))
                    .width(1.0);
                plot_ui.line(line);
            }
        });

        // Bottom row: main contour plot + F1 projection side by side
        ui.horizontal(|ui| {
            // Main 2D contour plot
            let main_plot = Plot::new("spectrum_2d")
                .height(main_h)
                .width(main_w)
                .x_axis_label(x_label.clone())
                .y_axis_label(y_label.clone())
                .allow_drag(true)
                .allow_zoom(true)
                .allow_scroll(true)
                .allow_boxed_zoom(true)
                .show_grid([true, true])
                .link_axis(link_id, [true, true]);

            let main_plot = if has_axes {
                let p = main_plot.x_axis_formatter(|val, _range| format!("{:.1}", -val.value));
                if has_y_axis {
                    p.y_axis_formatter(|val, _range| format!("{:.1}", -val.value))
                } else {
                    p
                }
            } else {
                main_plot
            };

            let pos_pts = pos_points.clone();
            let neg_pts = neg_points.clone();
            let pos_col = state.positive_color;
            let neg_col = state.negative_color;
            main_plot.show(ui, |plot_ui: &mut PlotUi| {
                if !pos_pts.is_empty() {
                    let pts = Points::new(PlotPoints::from(pos_pts))
                        .name("Positive")
                        .color(pos_col)
                        .radius(1.5);
                    plot_ui.points(pts);
                }
                if !neg_pts.is_empty() {
                    let pts = Points::new(PlotPoints::from(neg_pts))
                        .name("Negative")
                        .color(neg_col)
                        .radius(1.5);
                    plot_ui.points(pts);
                }
            });

            // Right: F1 projection (vertical 1D trace, rotated)
            if has_y_axis {
                let f1_plot = Plot::new("f1_projection")
                    .height(main_h)
                    .width(proj_width)
                    .show_axes([false, true])
                    .show_grid([false, true])
                    .allow_drag([false, true])
                    .allow_zoom(true)
                    .allow_scroll(true)
                    .allow_boxed_zoom(true)
                    .x_axis_label("")
                    .y_axis_label("")
                    .y_axis_formatter(|val, _range| format!("{:.1}", -val.value))
                    .link_axis(link_id, [false, true]);

                let f1_data = f1_proj.clone();
                f1_plot.show(ui, |plot_ui: &mut PlotUi| {
                    if !f1_data.is_empty() {
                        // Plot rotated: x = intensity, y = ppm
                        let rotated: Vec<[f64; 2]> = f1_data
                            .iter()
                            .map(|&[ppm, intensity]| [intensity, ppm])
                            .collect();
                        let line = Line::new(PlotPoints::from(rotated))
                            .color(egui::Color32::from_rgb(0x40, 0x80, 0xC0))
                            .width(1.0);
                        plot_ui.line(line);
                    }
                });
            }
        });
    } else {
        // ── No projections: single full-size contour plot ──
        let mut plot = Plot::new("spectrum_2d")
            .height(available_h)
            .x_axis_label(x_label)
            .y_axis_label(y_label)
            .allow_drag(true)
            .allow_zoom(true)
            .allow_scroll(true)
            .allow_boxed_zoom(true)
            .show_grid([true, true]);

        if has_axes {
            plot = plot.x_axis_formatter(|val, _range| format!("{:.1}", -val.value));
            if has_y_axis {
                plot = plot.y_axis_formatter(|val, _range| format!("{:.1}", -val.value));
            }
        }

        let pos_col = state.positive_color;
        let neg_col = state.negative_color;
        plot.show(ui, |plot_ui: &mut PlotUi| {
            if !pos_points.is_empty() {
                let pts = Points::new(PlotPoints::from(pos_points))
                    .name("Positive")
                    .color(pos_col)
                    .radius(1.5);
                plot_ui.points(pts);
            }
            if !neg_points.is_empty() {
                let pts = Points::new(PlotPoints::from(neg_points))
                    .name("Negative")
                    .color(neg_col)
                    .radius(1.5);
                plot_ui.points(pts);
            }
        });
    }

    request_ft
}
