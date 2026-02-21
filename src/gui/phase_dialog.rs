/// Interactive phase correction dialog
///
/// Provides click-and-drag phase adjustment:
/// - Horizontal drag → PH0 (zero-order)
/// - Vertical drag → PH1 (first-order)
/// - Real-time preview of phase-corrected spectrum

use crate::data::spectrum::SpectrumData;
use std::f64::consts::PI;

/// State for the interactive phase correction mode
#[derive(Debug, Clone)]
pub struct PhaseDialogState {
    pub active: bool,
    pub ph0: f64,
    pub ph1: f64,
    pub dragging: bool,
    pub drag_start: Option<egui::Pos2>,
    pub sensitivity_ph0: f64, // degrees per pixel
    pub sensitivity_ph1: f64,
    /// Preview spectrum (phased copy)
    pub preview: Vec<f64>,
}

impl Default for PhaseDialogState {
    fn default() -> Self {
        Self {
            active: false,
            ph0: 0.0,
            ph1: 0.0,
            dragging: false,
            drag_start: None,
            sensitivity_ph0: 0.5,
            sensitivity_ph1: 0.2,
            preview: Vec::new(),
        }
    }
}

impl PhaseDialogState {
    /// Apply phase to a spectrum's data (for preview, non-destructive)
    pub fn compute_preview(&mut self, spectrum: &SpectrumData) {
        let n = spectrum.real.len();
        if n == 0 {
            self.preview.clear();
            return;
        }

        self.preview.resize(n, 0.0);
        let ph0_rad = self.ph0 * PI / 180.0;
        let ph1_rad = self.ph1 * PI / 180.0;

        for i in 0..n {
            let frac = i as f64 / n as f64;
            let phase = ph0_rad + ph1_rad * frac;
            let re = spectrum.real[i];
            let im = if i < spectrum.imag.len() {
                spectrum.imag[i]
            } else {
                0.0
            };
            self.preview[i] = re * phase.cos() - im * phase.sin();
        }
    }
}

/// Show the interactive phase correction controls and handle drag input
pub fn show_phase_controls(
    ui: &mut egui::Ui,
    state: &mut PhaseDialogState,
) -> PhaseAction {
    let mut action = PhaseAction::None;

    ui.horizontal(|ui| {
        if state.active {
            ui.colored_label(
                egui::Color32::from_rgb(0x1B, 0x7A, 0x3D),
                "⟳ Interactive Phase Correction",
            );
            ui.separator();
            if ui.button("✅ Apply").clicked() {
                action = PhaseAction::Apply;
            }
            if ui.button("✖ Cancel").clicked() {
                action = PhaseAction::Cancel;
            }
        } else {
            if ui.button("⟳ Start Interactive Phasing").clicked() {
                state.active = true;
                action = PhaseAction::Start;
            }
        }
    });

    if state.active {
        ui.horizontal(|ui| {
            let ph0_changed = ui
                .add(
                    egui::Slider::new(&mut state.ph0, -360.0..=360.0)
                        .text("PH0 (°)")
                        .fixed_decimals(1),
                )
                .changed();
            let ph1_changed = ui
                .add(
                    egui::Slider::new(&mut state.ph1, -360.0..=360.0)
                        .text("PH1 (°)")
                        .fixed_decimals(1),
                )
                .changed();

            if ph0_changed || ph1_changed {
                action = PhaseAction::UpdatePreview;
            }
        });

        ui.label("Drag on spectrum: horizontal → PH0, vertical → PH1");
    }

    action
}

/// Actions from the phase dialog
#[derive(Debug, Clone, PartialEq)]
pub enum PhaseAction {
    None,
    Start,
    UpdatePreview,
    Apply,
    Cancel,
}
