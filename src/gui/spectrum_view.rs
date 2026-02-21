/// 1D Spectrum viewer widget — interactive plot with zoom/pan and ppm axis

use egui_plot::{Line, Plot, PlotPoints, PlotUi, Points, Text, VLine};

use crate::data::spectrum::{ExperimentType, Nucleus, SpectrumData};
use crate::gui::phase_dialog::PhaseDialogState;

/// State for the spectrum viewer
#[derive(Debug, Clone)]
pub struct SpectrumViewState {
    pub show_imaginary: bool,
    pub vertical_scale: f64,
    pub auto_scale: bool,
    pub baseline_picking: bool,
    pub baseline_points: Vec<[f64; 2]>,
    /// Detected peaks: [ppm, intensity]
    pub peaks: Vec<[f64; 2]>,
    pub show_peaks: bool,
    /// Manual peak picking mode
    pub peak_picking: bool,
    /// Detected multiplets
    pub multiplets: Vec<crate::pipeline::processing::Multiplet>,
    pub show_multiplets: bool,
    /// Integration regions: (start_ppm, end_ppm, raw_integral)
    pub integrations: Vec<(f64, f64, f64)>,
    pub show_integrations: bool,
    pub integration_picking: bool,
    pub integration_start: Option<f64>,
    /// Number of H for the reference (first) integral — user-settable
    pub integration_reference_h: f64,
    /// J-coupling measurement: pick two peaks to measure the distance
    pub j_coupling_picking: bool,
    pub j_coupling_first: Option<f64>, // ppm of first clicked peak
    /// Measured J-coupling results: (ppm1, ppm2, delta_ppm, j_hz)
    pub j_couplings: Vec<(f64, f64, f64, f64)>,
    pub show_j_couplings: bool,
    /// Incremented on auto-scale to give the plot a fresh ID (resets zoom)
    pub plot_generation: u32,
}

impl Default for SpectrumViewState {
    fn default() -> Self {
        Self {
            show_imaginary: false,
            vertical_scale: 1.0,
            auto_scale: true,
            baseline_picking: false,
            baseline_points: Vec::new(),
            peaks: Vec::new(),
            show_peaks: true,
            peak_picking: false,
            multiplets: Vec::new(),
            show_multiplets: true,
            integrations: Vec::new(),
            show_integrations: true,
            integration_picking: false,
            integration_start: None,
            integration_reference_h: 1.0,
            j_coupling_picking: false,
            j_coupling_first: None,
            j_couplings: Vec::new(),
            show_j_couplings: true,
            plot_generation: 0,
        }
    }
}

/// Default ppm display range for a given nucleus / experiment
fn default_ppm_range(spectrum: &SpectrumData) -> Option<(f64, f64)> {
    if !spectrum.is_frequency_domain {
        return None;
    }
    let nuc = spectrum.axes.first().map(|a| &a.nucleus);
    match nuc {
        Some(Nucleus::H1) => Some((-1.0, 14.0)),
        Some(Nucleus::C13) => Some((-10.0, 230.0)),
        Some(Nucleus::F19) => Some((-230.0, 30.0)),
        Some(Nucleus::P31) => Some((-50.0, 100.0)),
        Some(Nucleus::N15) => Some((0.0, 350.0)),
        _ => match &spectrum.experiment_type {
            ExperimentType::Proton => Some((-1.0, 14.0)),
            ExperimentType::Carbon | ExperimentType::Dept135 => Some((-10.0, 230.0)),
            _ => None,
        },
    }
}

/// Whether to clip negative display values.
/// Disabled by default — the user should phase-correct first.
/// DEPT-135 must never clip (negative peaks are real).
fn should_clip_negatives(_spectrum: &SpectrumData) -> bool {
    // Never clip by default.  After phasing, peaks point up and the
    // small negative noise is insignificant in a properly-scaled plot.
    // Force-clipping before phasing makes the spectrum invisible.
    false
}

/// Show the 1D spectrum plot with optional interactive phasing support
pub fn show_spectrum_1d(
    ui: &mut egui::Ui,
    spectrum: &SpectrumData,
    before_spectrum: Option<&SpectrumData>,
    state: &mut SpectrumViewState,
    show_before_after: bool,
    phase_state: &mut PhaseDialogState,
    colors: &super::theme::ThemeColors,
) {
    if spectrum.real.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.heading("No spectrum data loaded");
        });
        return;
    }

    let is_phasing = phase_state.active;

    // Controls above the plot
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.show_imaginary, "Imaginary");
        ui.separator();
        if ui.button("⊞ Auto Scale").clicked() {
            state.auto_scale = true;
        }
        ui.separator();
        ui.label(format!(
            "{} | {} pts | {}",
            spectrum.experiment_type,
            spectrum.real.len(),
            if spectrum.is_frequency_domain { "Freq" } else { "Time" }
        ));
        if !state.peaks.is_empty() {
            ui.separator();
            ui.checkbox(&mut state.show_peaks, &format!("📍 {} peaks", state.peaks.len()));
        }
        if !state.multiplets.is_empty() {
            ui.separator();
            ui.checkbox(&mut state.show_multiplets, &format!("🎵 {} multiplets", state.multiplets.len()));
        }
        if !state.integrations.is_empty() {
            ui.separator();
            ui.checkbox(
                &mut state.show_integrations,
                &format!("∫ {} regions", state.integrations.len()),
            );
        }
        if state.peak_picking {
            ui.separator();
            ui.colored_label(
                egui::Color32::from_rgb(0xE0, 0x30, 0x30),
                "🎯 Click to place peak (Shift+click to remove nearest)",
            );
        }
        if state.integration_picking {
            ui.separator();
            let msg = if state.integration_start.is_some() {
                "🎯 Click end point…"
            } else {
                "🎯 Click start point…"
            };
            ui.colored_label(egui::Color32::from_rgb(0x8B, 0x00, 0x8B), msg);
        }
        if state.j_coupling_picking {
            ui.separator();
            let msg = if state.j_coupling_first.is_some() {
                "📏 Click second peak…"
            } else {
                "📏 Click first peak…"
            };
            ui.colored_label(egui::Color32::from_rgb(0xCC, 0x66, 0x00), msg);
        }
        if !state.j_couplings.is_empty() {
            ui.separator();
            ui.checkbox(
                &mut state.show_j_couplings,
                &format!("📏 {} J", state.j_couplings.len()),
            );
        }
        if is_phasing {
            ui.separator();
            ui.colored_label(
                egui::Color32::from_rgb(0x27, 0x8B, 0x4A),
                format!("⟳ PH0={:.1}°  PH1={:.1}°  (drag: H→PH0, V→PH1)", phase_state.ph0, phase_state.ph1),
            );
        }
    });

    // Build ppm/point scale
    let raw_ppm = if spectrum.is_frequency_domain && !spectrum.axes.is_empty() {
        spectrum.axes[0].ppm_scale()
    } else {
        (0..spectrum.real.len())
            .map(|i| i as f64)
            .collect::<Vec<_>>()
    };

    // For frequency domain: negate ppm so high ppm appears on left in the plot
    // (egui_plot puts lower x on the left; negating flips the axis)
    let is_freq = spectrum.is_frequency_domain;
    let ppm_scale: Vec<f64> = if is_freq {
        raw_ppm.iter().map(|&x| -x).collect()
    } else {
        raw_ppm.clone()
    };

    let x_label = if is_freq {
        "Chemical Shift (ppm)"
    } else {
        "Point"
    };

    // Select which data to plot as the primary line
    let primary_data = if is_phasing && !phase_state.preview.is_empty() {
        &phase_state.preview
    } else {
        &spectrum.real
    };

    let clip_neg = should_clip_negatives(spectrum);

    // Primary spectrum line
    let real_points: PlotPoints = ppm_scale
        .iter()
        .zip(primary_data.iter())
        .map(|(&x, &y)| {
            let ys = y * state.vertical_scale;
            [x, if clip_neg { ys.max(0.0) } else { ys }]
        })
        .collect();

    let line_color = if is_phasing {
        colors.spectrum_phase
    } else {
        colors.spectrum_line
    };
    let real_line = Line::new(real_points)
        .name(if is_phasing { "Phased Preview" } else { "Real" })
        .color(line_color)
        .width(1.2);

    // Bump generation to reset internal plot view state on auto-scale
    if state.auto_scale {
        state.plot_generation = state.plot_generation.wrapping_add(1);
    }

    let no_interact = is_phasing || state.baseline_picking || state.integration_picking || state.j_coupling_picking || state.peak_picking;

    // X-axis: NMR convention — high ppm on left, low ppm on right
    let mut plot = Plot::new(format!("spectrum_1d_{}", state.plot_generation))
        .height(ui.available_height() - 4.0)
        .x_axis_label(x_label)
        .y_axis_label("")
        .allow_drag(!no_interact)
        .allow_zoom(true)
        .allow_scroll(true)
        .allow_boxed_zoom(!no_interact)
        .show_axes([true, false])
        .show_grid([true, false])
        .legend(egui_plot::Legend::default().position(egui_plot::Corner::RightTop)
            .background_alpha(0.6));

    // Format x labels as positive ppm (we negated the values for flipping)
    if is_freq {
        plot = plot.x_axis_formatter(|val, _range| {
            format!("{:.1}", -val.value)
        });
    }

    // Set default bounds on first display (auto_scale)
    if state.auto_scale && is_freq {
        if let Some((lo, hi)) = default_ppm_range(spectrum) {
            // Negate: high ppm → more-negative x, low ppm → less-negative x
            plot = plot
                .include_x(-lo)
                .include_x(-hi);
        }
    }
    // When clipping negatives, anchor the y-axis at 0 and show
    // only positive side (with a small margin above tallest peak)
    if clip_neg && is_freq {
        plot = plot.include_y(0.0);
    }
    state.auto_scale = false;

    // Clone state for use inside closure
    let bl_points_clone = state.baseline_points.clone();
    let is_picking_bl = state.baseline_picking;
    let peaks_clone = state.peaks.clone();
    let show_peaks_flag = state.show_peaks;
    let integrations_clone = state.integrations.clone();
    let show_integrations_flag = state.show_integrations;
    let multiplets_clone = state.multiplets.clone();
    let show_multiplets_flag = state.show_multiplets;
    let j_couplings_clone = state.j_couplings.clone();
    let show_j_couplings_flag = state.show_j_couplings;
    let vert_scale = state.vertical_scale;
    let ref_h = state.integration_reference_h;

    let plot_resp = plot.show(ui, |plot_ui: &mut PlotUi| {
        // When phasing, show original spectrum as faded background
        if is_phasing {
            let orig_points: PlotPoints = ppm_scale
                .iter()
                .zip(spectrum.real.iter())
                .map(|(&x, &y)| [x, y * vert_scale])
                .collect();
            let orig_line = Line::new(orig_points)
                .name("Original")
                .color(egui::Color32::from_rgba_premultiplied(170, 175, 190, 55))
                .width(0.8);
            plot_ui.line(orig_line);
        }

        plot_ui.line(real_line);

        // Imaginary part
        if state.show_imaginary && !spectrum.imag.is_empty() {
            let imag_points: PlotPoints = ppm_scale
                .iter()
                .zip(spectrum.imag.iter())
                .map(|(&x, &y)| [x, y * vert_scale])
                .collect();
            let imag_line = Line::new(imag_points)
                .name("Imaginary")
                .color(colors.spectrum_imaginary)
                .width(0.7);
            plot_ui.line(imag_line);
        }

        // Before spectrum overlay (faded) — only when not phasing
        if show_before_after && !is_phasing {
            if let Some(before) = before_spectrum {
                let before_ppm_raw = if before.is_frequency_domain && !before.axes.is_empty() {
                    before.axes[0].ppm_scale()
                } else {
                    (0..before.real.len())
                        .map(|i| i as f64)
                        .collect::<Vec<_>>()
                };
                let before_ppm: Vec<f64> = if before.is_frequency_domain {
                    before_ppm_raw.iter().map(|&x| -x).collect()
                } else {
                    before_ppm_raw
                };
                let before_points: PlotPoints = before_ppm
                    .iter()
                    .zip(before.real.iter())
                    .map(|(&x, &y)| [x, y * vert_scale])
                    .collect();
                let before_line = Line::new(before_points)
                    .name("Before")
                    .color(egui::Color32::from_rgba_premultiplied(140, 140, 150, 70))
                    .width(1.0);
                plot_ui.line(before_line);
            }
        }

        // ── Integration regions ──
        if show_integrations_flag && !integrations_clone.is_empty() {
            let fill_colors = [
                egui::Color32::from_rgba_premultiplied(76, 175, 80, 40),
                egui::Color32::from_rgba_premultiplied(33, 150, 243, 40),
                egui::Color32::from_rgba_premultiplied(255, 152, 0, 40),
                egui::Color32::from_rgba_premultiplied(156, 39, 176, 40),
                egui::Color32::from_rgba_premultiplied(244, 67, 54, 40),
            ];
            let border_colors = [
                egui::Color32::from_rgb(76, 175, 80),
                egui::Color32::from_rgb(33, 150, 243),
                egui::Color32::from_rgb(255, 152, 0),
                egui::Color32::from_rgb(156, 39, 176),
                egui::Color32::from_rgb(244, 67, 54),
            ];
            let first_raw = integrations_clone
                .first()
                .map(|r| r.2)
                .unwrap_or(1.0)
                .abs()
                .max(1e-12);

            for (idx, &(start_ppm, end_ppm, raw_val)) in integrations_clone.iter().enumerate() {
                let c = idx % fill_colors.len();
                let lo = start_ppm.min(end_ppm);
                let hi = start_ppm.max(end_ppm);

                // Filled area under the curve for this region
                let region_pts: Vec<[f64; 2]> = ppm_scale
                    .iter()
                    .zip(primary_data.iter())
                    .filter(|(&x, _)| {
                        let real_ppm = if is_freq { -x } else { x };
                        real_ppm >= lo && real_ppm <= hi
                    })
                    .map(|(&x, &y)| {
                        let ys = y * vert_scale;
                        [x, if clip_neg { ys.max(0.0) } else { ys }]
                    })
                    .collect();

                if !region_pts.is_empty() {
                    let fill_line = Line::new(PlotPoints::from(region_pts))
                        .color(fill_colors[c])
                        .fill(0.0)
                        .width(0.0)
                        .name(format!("Int {}", idx + 1));
                    plot_ui.line(fill_line);
                }

                // Boundary dashed lines
                let disp_lo = if is_freq { -hi } else { lo };
                let disp_hi = if is_freq { -lo } else { hi };
                plot_ui.vline(
                    VLine::new(disp_lo)
                        .color(border_colors[c])
                        .style(egui_plot::LineStyle::dashed_dense()),
                );
                plot_ui.vline(
                    VLine::new(disp_hi)
                        .color(border_colors[c])
                        .style(egui_plot::LineStyle::dashed_dense()),
                );

                // Integral value label centered in region
                let mid_ppm = (lo + hi) / 2.0;
                let disp_mid = if is_freq { -mid_ppm } else { mid_ppm };
                let max_y_in_region = primary_data
                    .iter()
                    .zip(ppm_scale.iter())
                    .filter(|(_, &x)| {
                        let rp = if is_freq { -x } else { x };
                        rp >= lo && rp <= hi
                    })
                    .map(|(&y, _)| y * vert_scale)
                    .fold(0.0f64, f64::max);
                let label_y = max_y_in_region * 1.08;
                let rel_val = (raw_val / first_raw) * ref_h;
                let label = Text::new(
                    [disp_mid, label_y].into(),
                    egui::RichText::new(format!("{:.2}H", rel_val))
                        .size(11.0)
                        .color(border_colors[c]),
                )
                .anchor(egui::Align2::CENTER_BOTTOM);
                plot_ui.text(label);
            }
        }

        // ── Peak markers and labels ──
        if show_peaks_flag && !peaks_clone.is_empty() {
            let peak_pts: PlotPoints = peaks_clone
                .iter()
                .map(|p| {
                    let x = if is_freq { -p[0] } else { p[0] };
                    let y = if clip_neg {
                        (p[1] * vert_scale).max(0.0)
                    } else {
                        p[1] * vert_scale
                    };
                    [x, y]
                })
                .collect();
            let markers = Points::new(peak_pts)
                .name("Peaks")
                .color(colors.peak_marker)
                .radius(2.5)
                .shape(egui_plot::MarkerShape::Down);
            plot_ui.points(markers);

            // Peak ppm labels above each marker
            for peak in &peaks_clone {
                let x = if is_freq { -peak[0] } else { peak[0] };
                let y = if clip_neg {
                    (peak[1] * vert_scale).max(0.0)
                } else {
                    peak[1] * vert_scale
                };
                let label = Text::new(
                    [x, y * 1.06].into(),
                    egui::RichText::new(format!("{:.2}", peak[0]))
                        .size(9.0)
                        .color(colors.peak_label),
                )
                .anchor(egui::Align2::CENTER_BOTTOM);
                plot_ui.text(label);
            }
        }

        // ── Multiplet labels ──
        if show_multiplets_flag && !multiplets_clone.is_empty() {
            // Find global max for consistent label positioning
            let global_max = primary_data
                .iter()
                .cloned()
                .fold(0.0f64, f64::max)
                * vert_scale;
            let label_base_y = -global_max * 0.06; // below the x-axis baseline

            for mult in &multiplets_clone {
                let cx = if is_freq { -mult.center_ppm } else { mult.center_ppm };

                // Build label text: "d" or "t, J=7.2"
                let lbl = if mult.j_hz > 0.5 {
                    format!("{}, J={:.1}", mult.label, mult.j_hz)
                } else {
                    mult.label.clone()
                };

                let label = Text::new(
                    [cx, label_base_y].into(),
                    egui::RichText::new(lbl)
                        .size(10.0)
                        .color(colors.multiplet_label),
                )
                .anchor(egui::Align2::CENTER_TOP);
                plot_ui.text(label);

                // Draw a small bracket line spanning the multiplet peaks
                if mult.peaks.len() >= 2 {
                    let lo_ppm = mult.peaks.first().unwrap()[0];
                    let hi_ppm = mult.peaks.last().unwrap()[0];
                    let x1 = if is_freq { -hi_ppm } else { lo_ppm };
                    let x2 = if is_freq { -lo_ppm } else { hi_ppm };
                    let bracket = Line::new(PlotPoints::from(vec![
                        [x1, label_base_y * 0.8],
                        [x1, label_base_y * 0.6],
                        [x2, label_base_y * 0.6],
                        [x2, label_base_y * 0.8],
                    ]))
                    .color(colors.multiplet_label)
                    .width(1.0);
                    plot_ui.line(bracket);
                }
            }
        }

        // ── J-coupling measurement lines ──
        if show_j_couplings_flag && !j_couplings_clone.is_empty() {
            for &(ppm1, ppm2, _delta_ppm, j_hz) in &j_couplings_clone {
                let x1 = if is_freq { -ppm1 } else { ppm1 };
                let x2 = if is_freq { -ppm2 } else { ppm2 };

                // Find the intensity at each ppm to position the line
                let y1 = find_intensity_at_ppm(primary_data, &ppm_scale, x1) * vert_scale;
                let y2 = find_intensity_at_ppm(primary_data, &ppm_scale, x2) * vert_scale;
                let bar_y = y1.max(y2) * 1.12;

                // Draw a horizontal bar connecting the two peaks
                let bar = Line::new(PlotPoints::from(vec![
                    [x1, bar_y],
                    [x2, bar_y],
                ]))
                .color(colors.j_coupling_color)
                .width(1.5);
                plot_ui.line(bar);

                // Vertical tick marks at each end
                let tick_h = bar_y * 0.03;
                let t1 = Line::new(PlotPoints::from(vec![
                    [x1, bar_y - tick_h],
                    [x1, bar_y + tick_h],
                ]))
                .color(colors.j_coupling_color)
                .width(1.5);
                plot_ui.line(t1);
                let t2 = Line::new(PlotPoints::from(vec![
                    [x2, bar_y - tick_h],
                    [x2, bar_y + tick_h],
                ]))
                .color(colors.j_coupling_color)
                .width(1.5);
                plot_ui.line(t2);

                // Label: "J = X.X Hz"
                let mid_x = (x1 + x2) / 2.0;
                let label = Text::new(
                    [mid_x, bar_y * 1.03].into(),
                    egui::RichText::new(format!("J = {:.1} Hz", j_hz))
                        .size(10.0)
                        .color(colors.j_coupling_color),
                )
                .anchor(egui::Align2::CENTER_BOTTOM);
                plot_ui.text(label);
            }
        }

        // ── Baseline anchor points ──
        if !bl_points_clone.is_empty() {
            let pts: PlotPoints = bl_points_clone
                .iter()
                .map(|p| [if is_freq { -p[0] } else { p[0] }, p[1]])
                .collect();
            let markers = Points::new(pts)
                .name("Baseline Points")
                .color(colors.baseline_marker)
                .radius(5.0)
                .shape(egui_plot::MarkerShape::Diamond);
            plot_ui.points(markers);

            // Draw interpolated baseline as a line if ≥2 points
            if bl_points_clone.len() >= 2 {
                let mut sorted = bl_points_clone.clone();
                sorted.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap());
                let sorted_display: Vec<[f64; 2]> = sorted
                    .iter()
                    .map(|p| [if is_freq { -p[0] } else { p[0] }, p[1]])
                    .collect();
                let bl_line = Line::new(PlotPoints::from(sorted_display))
                    .name("Baseline")
                    .color(colors.baseline_marker)
                    .width(1.5)
                    .style(egui_plot::LineStyle::dashed_dense());
                plot_ui.line(bl_line);
            }

            // Draw vertical guidelines at each baseline point
            for pt in &bl_points_clone {
                let display_x = if is_freq { -pt[0] } else { pt[0] };
                plot_ui.vline(
                    VLine::new(display_x)
                        .color(egui::Color32::from_rgba_premultiplied(0xD4, 0x3F, 0x00, 40))
                );
            }
        }
    });

    // ── Handle clicks: baseline picking + integration picking + J-coupling + peak picking ──
    let any_picking = is_picking_bl || state.integration_picking || state.j_coupling_picking || state.peak_picking;
    if any_picking {
        if let Some(pos) = plot_resp.response.hover_pos() {
            if plot_resp.response.clicked() {
                let coord = plot_resp.transform.value_from_position(pos);
                // Store in real ppm (un-negate the x if freq domain)
                let real_x = if is_freq { -coord.x } else { coord.x };
                let shift_held = ui.input(|i| i.modifiers.shift);

                if is_picking_bl {
                    state.baseline_points.push([real_x, coord.y]);
                }

                if state.peak_picking {
                    if shift_held {
                        // Shift+click: remove nearest peak within tolerance
                        remove_nearest_peak(&mut state.peaks, real_x, 0.1);
                    } else {
                        // Normal click: add peak at nearest local maximum
                        let peak = find_nearest_local_max(spectrum, real_x, &raw_ppm);
                        state.peaks.push(peak);
                        // Re-sort peaks by ppm descending
                        state.peaks.sort_by(|a, b| b[0].partial_cmp(&a[0]).unwrap());
                    }
                }

                if state.integration_picking {
                    if let Some(start) = state.integration_start.take() {
                        // Second click → compute integral
                        let lo = start.min(real_x);
                        let hi = start.max(real_x);
                        let raw_integral =
                            crate::pipeline::processing::integrate_region(spectrum, lo, hi);
                        state.integrations.push((lo, hi, raw_integral));
                    } else {
                        // First click → mark start
                        state.integration_start = Some(real_x);
                    }
                }

                if state.j_coupling_picking {
                    // Snap to nearest detected peak if possible
                    let snapped = snap_to_nearest_peak(real_x, &state.peaks, 0.05);
                    if let Some(first_ppm) = state.j_coupling_first.take() {
                        // Second click → measure J
                        let delta_ppm = (snapped - first_ppm).abs();
                        let obs_mhz = spectrum
                            .axes
                            .first()
                            .map(|a| a.observe_freq_mhz)
                            .unwrap_or(400.0);
                        let j_hz = delta_ppm * obs_mhz;
                        state.j_couplings.push((first_ppm, snapped, delta_ppm, j_hz));
                    } else {
                        // First click
                        state.j_coupling_first = Some(snapped);
                    }
                }
            }
        }
    }

    // Handle drag for interactive phasing
    if is_phasing && plot_resp.response.dragged() {
        let delta = plot_resp.response.drag_delta();
        if delta.x.abs() > 0.1 || delta.y.abs() > 0.1 {
            phase_state.ph0 += delta.x as f64 * phase_state.sensitivity_ph0;
            phase_state.ph0 = phase_state.ph0.clamp(-360.0, 360.0);
            phase_state.ph1 -= delta.y as f64 * phase_state.sensitivity_ph1;
            phase_state.ph1 = phase_state.ph1.clamp(-360.0, 360.0);
            phase_state.compute_preview(spectrum);
        }
    }
}

/// Find the intensity at the nearest data point to a given display x-coordinate.
fn find_intensity_at_ppm(data: &[f64], ppm_scale: &[f64], display_x: f64) -> f64 {
    if data.is_empty() || ppm_scale.is_empty() {
        return 0.0;
    }
    let mut best_idx = 0;
    let mut best_dist = f64::MAX;
    for (i, &px) in ppm_scale.iter().enumerate() {
        let dist = (px - display_x).abs();
        if dist < best_dist {
            best_dist = dist;
            best_idx = i;
        }
    }
    if best_idx < data.len() {
        data[best_idx]
    } else {
        0.0
    }
}

/// Snap a clicked ppm to the nearest detected peak within `tolerance` ppm.
fn snap_to_nearest_peak(ppm: f64, peaks: &[[f64; 2]], tolerance: f64) -> f64 {
    if peaks.is_empty() {
        return ppm;
    }
    let mut best = ppm;
    let mut best_dist = f64::MAX;
    for peak in peaks {
        let dist = (peak[0] - ppm).abs();
        if dist < best_dist {
            best_dist = dist;
            best = peak[0];
        }
    }
    if best_dist <= tolerance {
        best
    } else {
        ppm
    }
}

/// Find the nearest local maximum to the clicked ppm position.
/// Returns [ppm, intensity] of the nearest local max.
fn find_nearest_local_max(
    spectrum: &SpectrumData,
    clicked_ppm: f64,
    ppm_scale: &[f64],
) -> [f64; 2] {
    let n = spectrum.real.len().min(ppm_scale.len());
    if n < 3 {
        return [clicked_ppm, 0.0];
    }

    // Find the index closest to clicked_ppm
    let mut closest_idx = 0;
    let mut closest_dist = f64::MAX;
    for (i, &ppm) in ppm_scale.iter().enumerate().take(n) {
        let dist = (ppm - clicked_ppm).abs();
        if dist < closest_dist {
            closest_dist = dist;
            closest_idx = i;
        }
    }

    // Search a window around the closest point for a local maximum
    let window = 20; // search ±20 data points
    let lo = closest_idx.saturating_sub(window);
    let hi = (closest_idx + window).min(n - 1);

    let mut best_idx = closest_idx;
    let mut best_val = spectrum.real[closest_idx];
    for i in lo..=hi {
        if spectrum.real[i] > best_val {
            best_val = spectrum.real[i];
            best_idx = i;
        }
    }

    [ppm_scale[best_idx], spectrum.real[best_idx]]
}

/// Remove the nearest peak within `tolerance` ppm of the clicked position.
fn remove_nearest_peak(peaks: &mut Vec<[f64; 2]>, ppm: f64, tolerance: f64) {
    if peaks.is_empty() {
        return;
    }
    let mut best_idx = 0;
    let mut best_dist = f64::MAX;
    for (i, peak) in peaks.iter().enumerate() {
        let dist = (peak[0] - ppm).abs();
        if dist < best_dist {
            best_dist = dist;
            best_idx = i;
        }
    }
    if best_dist <= tolerance {
        peaks.remove(best_idx);
    }
}
