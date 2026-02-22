/// Processing pipeline panel — left sidebar with processing controls

use crate::pipeline::processing::WindowFunction;

/// State for the pipeline panel UI
#[derive(Debug, Clone)]
pub struct PipelinePanelState {
    // Apodization
    pub apod_type: usize, // 0=None, 1=EM, 2=GM, 3=SineBell, 4=CosineBell
    pub em_lb: f64,
    pub gm_gb: f64,
    pub gm_lb: f64,
    pub sp_power: f64,
    pub sp_offset: f64,
    pub sp_end: f64,

    // Zero fill
    pub zf_factor: usize, // multiply current size by 2^factor

    // Phase
    pub ph0: f64,
    pub ph1: f64,

    // Peak detection
    pub peak_threshold: f64, // 0.0–1.0 fraction of max
    pub min_peak_spacing_hz: f64, // minimum Hz between peaks (lower = more peaks)

    // FT configuration
    pub ft_use_imaginary: bool,

    // Solvent suppression
    pub solvent_preset: usize, // 0=Custom, 1..N = preset solvents
    pub solvent_center: f64,
    pub solvent_width: f64,

    // State tracking
    pub show_before_after: bool,
}

impl Default for PipelinePanelState {
    fn default() -> Self {
        Self {
            apod_type: 1, // Default to EM
            em_lb: 0.3,
            gm_gb: 0.1,
            gm_lb: -1.0,
            sp_power: 2.0,
            sp_offset: 0.5,
            sp_end: 1.0,
            zf_factor: 1,
            ph0: 0.0,
            ph1: 0.0,
            peak_threshold: 0.05,
            min_peak_spacing_hz: 5.0,
            ft_use_imaginary: true,
            solvent_preset: 0, // Custom
            solvent_center: 4.7, // Water
            solvent_width: 0.1,
            show_before_after: false,
        }
    }
}

/// Actions triggered by the pipeline panel
#[derive(Debug, Clone, PartialEq)]
pub enum PipelineAction {
    None,
    ApplyApodization,
    ApplyZeroFill,
    ApplyFT,
    ApplyFT2D,
    ApplyPhaseCorrection,
    ApplyAutoPhase,
    ApplyBaselineCorrection,
    ApplyManualBaseline,
    ToggleBaselinePicking,
    ClearBaselinePoints,
    ApplySolventSuppression,
    DetectPeaks,
    ClearPeaks,
    TogglePeakPicking,
    RemoveLastPeak,
    DetectMultiplets,
    ClearMultiplets,
    ToggleJCouplingPicking,
    ClearJCouplings,
    ToggleIntegrationPicking,
    ClearIntegrations,
}

/// Picking mode states passed from the spectrum view, so buttons can be highlighted
pub struct PickingModes {
    pub peak_picking: bool,
    pub baseline_picking: bool,
    pub integration_picking: bool,
    pub j_coupling_picking: bool,
}

/// Render the pipeline panel in the left sidebar
pub fn show_pipeline_panel(
    ui: &mut egui::Ui,
    state: &mut PipelinePanelState,
    has_data: bool,
    is_freq_domain: bool,
    is_2d: bool,
    operation_count: usize,
    picking: &PickingModes,
    integration_ref_h: &mut f64,
    has_before_snapshot: bool,
) -> PipelineAction {
    let mut action = PipelineAction::None;

    ui.vertical_centered(|ui| {
        ui.heading("⚙️ Processing");
    });
    ui.separator();

    if !has_data {
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("Load NMR data to begin.")
                .size(12.5)
                .color(egui::Color32::from_rgb(0x88, 0x8C, 0x94)),
        );
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Drag & drop or File → Open")
                .size(12.0)
                .color(egui::Color32::from_rgb(0xAA, 0xAE, 0xB4)),
        );
        return action;
    }

    ui.label(
        egui::RichText::new(format!("📝 {} ops", operation_count))
            .size(11.5)
            .color(egui::Color32::from_rgb(0x66, 0x6C, 0x78)),
    );
    ui.add_space(4.0);
    ui.separator();

    // ── Time Domain Operations ──
    if !is_freq_domain {
        ui.collapsing("📊 Apodization", |ui| {
            egui::ComboBox::from_label("Window Function")
                .selected_text(match state.apod_type {
                    0 => "None",
                    1 => "Exponential (EM)",
                    2 => "Gaussian (GM)",
                    3 => "Sine Bell (SP)",
                    4 => "Cosine Bell",
                    _ => "Unknown",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut state.apod_type, 0, "None");
                    ui.selectable_value(&mut state.apod_type, 1, "Exponential (EM)");
                    ui.selectable_value(&mut state.apod_type, 2, "Gaussian (GM)");
                    ui.selectable_value(&mut state.apod_type, 3, "Sine Bell (SP)");
                    ui.selectable_value(&mut state.apod_type, 4, "Cosine Bell");
                });

            match state.apod_type {
                1 => {
                    ui.add(
                        egui::Slider::new(&mut state.em_lb, 0.0..=20.0)
                            .text("LB (Hz)")
                            .fixed_decimals(1),
                    );
                }
                2 => {
                    ui.add(
                        egui::Slider::new(&mut state.gm_gb, 0.001..=1.0)
                            .text("GB")
                            .fixed_decimals(3),
                    );
                    ui.add(
                        egui::Slider::new(&mut state.gm_lb, -10.0..=10.0)
                            .text("LB (Hz)")
                            .fixed_decimals(1),
                    );
                }
                3 => {
                    ui.add(
                        egui::Slider::new(&mut state.sp_power, 0.5..=4.0)
                            .text("Power")
                            .fixed_decimals(1),
                    );
                    ui.add(
                        egui::Slider::new(&mut state.sp_offset, 0.0..=1.0)
                            .text("Offset")
                            .fixed_decimals(2),
                    );
                    ui.add(
                        egui::Slider::new(&mut state.sp_end, 0.0..=1.0)
                            .text("End")
                            .fixed_decimals(2),
                    );
                }
                _ => {}
            }

            if state.apod_type > 0 && ui.button("▶ Apply Apodization").clicked() {
                action = PipelineAction::ApplyApodization;
            }
        });

        ui.collapsing("📏 Zero Fill", |ui| {
            ui.add(
                egui::Slider::new(&mut state.zf_factor, 1..=4)
                    .text("Factor (×2^n)")
            );
            if ui.button("▶ Apply Zero Fill").clicked() {
                action = PipelineAction::ApplyZeroFill;
            }
        });

        ui.separator();
        if is_2d {
            // 2D Fourier Transform
            if ui.button("🔄 2D Fourier Transform").clicked() {
                action = PipelineAction::ApplyFT2D;
            }
            ui.label(
                egui::RichText::new("Applies complex FFT along F2 then F1,\nresult in magnitude mode.")
                    .size(11.0)
                    .color(egui::Color32::from_rgb(0x88, 0x8C, 0x94)),
            );
        } else {
            // 1D Fourier Transform
            ui.checkbox(&mut state.ft_use_imaginary, "Use imaginary data (complex FFT)");
            if ui.button("🔄 Fourier Transform").clicked() {
                action = PipelineAction::ApplyFT;
            }
        }
    }

    // ── Frequency Domain Operations ──
    if is_freq_domain {
        ui.collapsing("🔧 Phase Correction", |ui| {
            ui.add(
                egui::Slider::new(&mut state.ph0, -360.0..=360.0)
                    .text("PH0 (°)")
                    .fixed_decimals(1),
            );
            ui.add(
                egui::Slider::new(&mut state.ph1, -360.0..=360.0)
                    .text("PH1 (°)")
                    .fixed_decimals(1),
            );
            ui.horizontal(|ui| {
                if ui.button("▶ Apply").clicked() {
                    action = PipelineAction::ApplyPhaseCorrection;
                }
                if ui.button("🤖 Auto Phase").clicked() {
                    action = PipelineAction::ApplyAutoPhase;
                }
            });
            ui.label("💡 Tip: Click & drag on spectrum for interactive phasing");
        });

        ui.collapsing("📐 Baseline Correction", |ui| {
            if ui.button("▶ Auto Baseline").clicked() {
                action = PipelineAction::ApplyBaselineCorrection;
            }
            ui.separator();
            ui.label("Manual baseline:");
            ui.label("Click points on the spectrum to define");
            ui.label("the baseline, then apply.");
            ui.horizontal(|ui| {
                let bl_label = if picking.baseline_picking { "🎯 Picking ●" } else { "🎯 Pick Points" };
                let bl_btn = egui::Button::new(
                    egui::RichText::new(bl_label)
                        .color(if picking.baseline_picking { egui::Color32::WHITE } else { ui.visuals().text_color() })
                )
                .fill(if picking.baseline_picking { egui::Color32::from_rgb(0x00, 0x99, 0x66) } else { ui.visuals().widgets.inactive.bg_fill });
                if ui.add(bl_btn).clicked() {
                    action = PipelineAction::ToggleBaselinePicking;
                }
                if ui.button("✕ Clear").clicked() {
                    action = PipelineAction::ClearBaselinePoints;
                }
            });
            if ui.button("▶ Apply Manual Baseline").clicked() {
                action = PipelineAction::ApplyManualBaseline;
            }
        });

        ui.collapsing("🧪 Solvent Suppression", |ui| {
            // Solvent presets
            let presets = [
                ("Custom",           0.0,    0.0),
                ("CDCl\u{2083} (7.26 ppm)",  7.26, 0.08),
                ("DMSO-d\u{2086} (2.50 ppm)", 2.50, 0.08),
                ("D\u{2082}O (4.79 ppm)",     4.79, 0.15),
                ("MeOD (3.31 ppm)",  3.31, 0.08),
                ("Acetone-d\u{2086} (2.05 ppm)", 2.05, 0.08),
                ("C\u{2086}D\u{2086} (7.16 ppm)", 7.16, 0.08),
                ("Water (4.70 ppm)", 4.70, 0.15),
            ];
            egui::ComboBox::from_label("Solvent")
                .selected_text(presets.get(state.solvent_preset).map(|p| p.0).unwrap_or("Custom"))
                .show_ui(ui, |ui| {
                    for (i, (name, _center, _width)) in presets.iter().enumerate() {
                        if ui.selectable_value(&mut state.solvent_preset, i, *name).clicked() {
                            if i > 0 {
                                state.solvent_center = presets[i].1;
                                state.solvent_width = presets[i].2;
                            }
                        }
                    }
                });
            ui.add(
                egui::Slider::new(&mut state.solvent_center, 0.0..=15.0)
                    .text("Center (ppm)")
                    .fixed_decimals(2),
            );
            ui.add(
                egui::Slider::new(&mut state.solvent_width, 0.01..=1.0)
                    .text("Width (ppm)")
                    .fixed_decimals(2),
            );
            if ui.button("▶ Apply Solvent Suppression").clicked() {
                action = PipelineAction::ApplySolventSuppression;
            }
        });

        ui.collapsing("📍 Peak Detection", |ui| {
            ui.add(
                egui::Slider::new(&mut state.peak_threshold, 0.01..=0.50)
                    .text("Threshold")
                    .fixed_decimals(2),
            );
            ui.add(
                egui::Slider::new(&mut state.min_peak_spacing_hz, 1.0..=100.0)
                    .text("Min spacing (Hz)")
                    .fixed_decimals(1),
            );
            ui.horizontal(|ui| {
                if ui.button("▶ Detect Peaks").clicked() {
                    action = PipelineAction::DetectPeaks;
                }
                if ui.button("✕ Clear").clicked() {
                    action = PipelineAction::ClearPeaks;
                }
            });
            ui.separator();
            ui.label("✋ Manual peak picking:");
            ui.horizontal(|ui| {
                let pk_label = if picking.peak_picking { "🎯 Picking ●" } else { "🎯 Pick Peaks" };
                let pk_btn = egui::Button::new(
                    egui::RichText::new(pk_label)
                        .color(if picking.peak_picking { egui::Color32::WHITE } else { ui.visuals().text_color() })
                )
                .fill(if picking.peak_picking { egui::Color32::from_rgb(0xD0, 0x30, 0x30) } else { ui.visuals().widgets.inactive.bg_fill });
                if ui.add(pk_btn).clicked() {
                    action = PipelineAction::TogglePeakPicking;
                }
                if ui.button("⌫ Remove Last").clicked() {
                    action = PipelineAction::RemoveLastPeak;
                }
            });
            ui.separator();
            ui.label("🎵 Multiplet analysis:");
            ui.horizontal(|ui| {
                if ui.button("▶ Detect Multiplets").clicked() {
                    action = PipelineAction::DetectMultiplets;
                }
                if ui.button("✕ Clear").clicked() {
                    action = PipelineAction::ClearMultiplets;
                }
            });
            ui.separator();
            ui.label("📏 J-Coupling measurement:");
            ui.label("Click two peaks to measure J.");
            ui.horizontal(|ui| {
                let j_label = if picking.j_coupling_picking { "📏 Measuring ●" } else { "📏 Measure J" };
                let j_btn = egui::Button::new(
                    egui::RichText::new(j_label)
                        .color(if picking.j_coupling_picking { egui::Color32::WHITE } else { ui.visuals().text_color() })
                )
                .fill(if picking.j_coupling_picking { egui::Color32::from_rgb(0xCC, 0x66, 0x00) } else { ui.visuals().widgets.inactive.bg_fill });
                if ui.add(j_btn).clicked() {
                    action = PipelineAction::ToggleJCouplingPicking;
                }
                if ui.button("✕ Clear").clicked() {
                    action = PipelineAction::ClearJCouplings;
                }
            });
        });

        ui.collapsing("∫ Integration", |ui| {
            ui.label("Click two points on the spectrum");
            ui.label("to define an integration region.");
            ui.horizontal(|ui| {
                let int_label = if picking.integration_picking { "🎯 Picking ●" } else { "🎯 Pick Region" };
                let int_btn = egui::Button::new(
                    egui::RichText::new(int_label)
                        .color(if picking.integration_picking { egui::Color32::WHITE } else { ui.visuals().text_color() })
                )
                .fill(if picking.integration_picking { egui::Color32::from_rgb(0x8B, 0x00, 0x8B) } else { ui.visuals().widgets.inactive.bg_fill });
                if ui.add(int_btn).clicked() {
                    action = PipelineAction::ToggleIntegrationPicking;
                }
                if ui.button("✕ Clear All").clicked() {
                    action = PipelineAction::ClearIntegrations;
                }
            });
            ui.add_space(4.0);
            ui.label("Reference H count (first region):");
            ui.add(
                egui::DragValue::new(integration_ref_h)
                    .speed(0.1)
                    .range(0.1..=100.0)
                    .suffix(" H")
                    .fixed_decimals(1),
            );
        });
    }

    ui.separator();

    // Before/After toggle — only show when a snapshot exists
    if has_before_snapshot {
        ui.checkbox(&mut state.show_before_after, "👁 Show Before/After");
    }

    action
}

/// Get the window function from the panel state
pub fn get_window_function(state: &PipelinePanelState) -> WindowFunction {
    match state.apod_type {
        1 => WindowFunction::Exponential { lb_hz: state.em_lb },
        2 => WindowFunction::Gaussian {
            gb: state.gm_gb,
            lb_hz: state.gm_lb,
        },
        3 => WindowFunction::SineBell {
            power: state.sp_power,
            offset: state.sp_offset,
            end: state.sp_end,
        },
        4 => WindowFunction::CosineBell,
        _ => WindowFunction::None,
    }
}
