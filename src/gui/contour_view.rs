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

/// Compute the F2 projection (max absolute value per column) and F1 projection (per row).
///
/// F2 projection returns `[-ppm_x, intensity]` — X matches contour X, Y = intensity.
/// F1 projection returns `[intensity, ppm_y]` — Y matches contour Y, X = intensity.
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
        // X = -ppm (matches contour X), Y = intensity
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
        // X = intensity, Y = +ppm (matches contour Y)
        f1_proj.push([max_val, y]);
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
    // X axis: -ppm so high ppm is on the LEFT (NMR convention)
    // Y axis: +ppm so high ppm is at the TOP (NMR convention)
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
                    pos_points.push([-x, y]);
                } else {
                    neg_points.push([-x, y]);
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

    let has_axes = !spectrum.axes.is_empty();
    let has_y_axis = spectrum.axes.len() >= 2;

    // X formatter: negate back to show positive ppm
    // Y formatter: already positive, show as-is
    let x_fmt = |val: egui_plot::GridMark, _range: &std::ops::RangeInclusive<f64>| {
        format!("{:.1}", -val.value)
    };
    let y_fmt = |val: egui_plot::GridMark, _range: &std::ops::RangeInclusive<f64>| {
        format!("{:.1}", val.value)
    };

    let pos_col = state.positive_color;
    let neg_col = state.negative_color;

    if state.show_projections {
        let proj_height = 100.0;
        let proj_width = 100.0;
        let available_h = ui.available_height() - 4.0;
        let available_w = ui.available_width() - 4.0;
        let main_h = (available_h - proj_height - 8.0).max(100.0);
        let main_w = if has_y_axis {
            (available_w - proj_width - 8.0).max(100.0)
        } else {
            available_w
        };

        let link_id = egui::Id::new("contour_link");
        let y_axis_w = 50.0; // fixed Y-axis width for alignment

        // ── Top: F2 projection ──
        // Show Y axis (with empty labels) to reserve space matching main contour
        let f2_plot = Plot::new("f2_projection")
            .height(proj_height)
            .width(main_w)
            .show_axes([false, true])
            .show_grid([false, false])
            .y_axis_formatter(|_, _| String::new())
            .y_axis_min_width(y_axis_w)
            .allow_drag([true, false])
            .allow_zoom([true, false])
            .allow_scroll([true, false])
            .allow_boxed_zoom(false)
            .x_axis_label("")
            .y_axis_label("")
            .link_axis(link_id, [true, false]);

        let f2_data = f2_proj.clone();
        f2_plot.show(ui, |plot_ui: &mut PlotUi| {
            if !f2_data.is_empty() {
                let line = Line::new(PlotPoints::from(f2_data))
                    .color(egui::Color32::from_rgb(0x40, 0x80, 0xC0))
                    .width(1.0)
                    .name("F2 projection");
                plot_ui.line(line);
            }
        });

        // ── Bottom row: main contour + F1 projection ──
        ui.horizontal(|ui| {
            // Main 2D contour plot
            let mut main_plot = Plot::new("spectrum_2d")
                .height(main_h)
                .width(main_w)
                .x_axis_label(x_label.clone())
                .y_axis_label(y_label.clone())
                .y_axis_min_width(y_axis_w)
                .allow_drag(true)
                .allow_zoom(true)
                .allow_scroll(true)
                .allow_boxed_zoom(true)
                .show_grid([true, true])
                .link_axis(link_id, [true, true]);

            if has_axes {
                main_plot = main_plot.x_axis_formatter(x_fmt);
                if has_y_axis {
                    main_plot = main_plot.y_axis_formatter(y_fmt);
                }
            }

            let pos_pts = pos_points.clone();
            let neg_pts = neg_points.clone();
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

            // F1 projection (right side)
            if has_y_axis {
                let f1_plot = Plot::new("f1_projection")
                    .height(main_h)
                    .width(proj_width)
                    .show_axes([false, false])
                    .show_grid([false, false])
                    .allow_drag([false, true])
                    .allow_zoom([false, true])
                    .allow_scroll([false, true])
                    .allow_boxed_zoom(false)
                    .x_axis_label("")
                    .y_axis_label("")
                    .link_axis(link_id, [false, true]);

                let f1_data = f1_proj.clone();
                f1_plot.show(ui, |plot_ui: &mut PlotUi| {
                    if !f1_data.is_empty() {
                        // Data is already [intensity, ppm_y]
                        let line = Line::new(PlotPoints::from(f1_data))
                            .color(egui::Color32::from_rgb(0x40, 0x80, 0xC0))
                            .width(1.0)
                            .name("F1 projection");
                        plot_ui.line(line);
                    }
                });
            }
        });
    } else {
        // ── No projections: single full-size contour plot ──
        let available_h = ui.available_height() - 4.0;
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
            plot = plot.x_axis_formatter(x_fmt);
            if has_y_axis {
                plot = plot.y_axis_formatter(y_fmt);
            }
        }

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
