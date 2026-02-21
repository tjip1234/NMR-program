/// 2D Contour plot viewer for 2D NMR experiments (COSY, HSQC, HMBC)

use egui_plot::{Plot, PlotPoints, Points, PlotUi};

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

/// Show a 2D spectrum as a scatter/contour plot
pub fn show_spectrum_2d(
    ui: &mut egui::Ui,
    spectrum: &SpectrumData,
    state: &mut ContourViewState,
) {
    if spectrum.data_2d.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.heading("No 2D spectrum data loaded");
        });
        return;
    }

    // Controls
    ui.horizontal(|ui| {
        ui.label(format!("{} | 2D", spectrum.experiment_type));
        ui.separator();
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
    });

    let n_rows = spectrum.data_2d.len();
    let n_cols = if n_rows > 0 {
        spectrum.data_2d[0].len()
    } else {
        0
    };

    // Find the maximum value for normalization
    let max_val = spectrum
        .data_2d
        .iter()
        .flat_map(|row| row.iter())
        .map(|v| v.abs())
        .fold(0.0f64, f64::max);

    if max_val == 0.0 {
        ui.label("All zero data");
        return;
    }

    let threshold_abs = state.threshold * max_val;

    // Collect points above threshold
    let mut pos_points: Vec<[f64; 2]> = Vec::new();
    let mut neg_points: Vec<[f64; 2]> = Vec::new();

    for row_idx in 0..n_rows {
        for col_idx in 0..n_cols {
            let val = spectrum.data_2d[row_idx][col_idx];
            if val.abs() > threshold_abs {
                // Map to ppm coordinates
                let x = if spectrum.axes.len() >= 1 {
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
    let x_label = if spectrum.axes.len() >= 1 {
        format!("{} (ppm)", spectrum.axes[0].label)
    } else {
        "F2 (points)".to_string()
    };
    let y_label = if spectrum.axes.len() >= 2 {
        format!("{} (ppm)", spectrum.axes[1].label)
    } else {
        "F1 (points)".to_string()
    };

    let mut plot = Plot::new("spectrum_2d")
        .height(ui.available_height() - 4.0)
        .x_axis_label(x_label)
        .y_axis_label(y_label)
        .allow_drag(true)
        .allow_zoom(true)
        .allow_scroll(true)
        .allow_boxed_zoom(true)
        .show_grid([true, true])
        .data_aspect(1.0);

    // Format x/y labels as positive ppm (we negated the values for flipping)
    if !spectrum.axes.is_empty() {
        plot = plot.x_axis_formatter(|val, _range| {
            format!("{:.1}", -val.value)
        });
        if spectrum.axes.len() >= 2 {
            plot = plot.y_axis_formatter(|val, _range| {
                format!("{:.1}", -val.value)
            });
        }
    }

    plot.show(ui, |plot_ui: &mut PlotUi| {
        if !pos_points.is_empty() {
            let pts = Points::new(PlotPoints::from(pos_points))
                .name("Positive")
                .color(state.positive_color)
                .radius(1.5);
            plot_ui.points(pts);
        }
        if !neg_points.is_empty() {
            let pts = Points::new(PlotPoints::from(neg_points))
                .name("Negative")
                .color(state.negative_color)
                .radius(1.5);
            plot_ui.points(pts);
        }
    });
}
