/// Main application state and eframe::App implementation
///
/// Ties together all subsystems: data, pipeline, GUI, and logging.

use std::path::PathBuf;

use eframe::egui;

use crate::data::spectrum::SpectrumData;
use crate::gui::contour_view::{self, ContourViewState};
use crate::gui::conversion_dialog::{
    self, ConversionAction, ConversionDialogState,
};
use crate::gui::export_dialog::{self, ExportAction, ExportDialogState, ExportSettings};
use crate::gui::export_tab::{self, ExportTabAction, ExportTabState};
use crate::gui::phase_dialog::{self, PhaseAction, PhaseDialogState};
use crate::gui::pipeline_panel::{self, PipelineAction, PipelinePanelState};
use crate::gui::spectrum_view::{self, SpectrumViewState};
use crate::gui::theme::{self, AppTheme, ThemeColors};
use crate::gui::toolbar::{self, ToolbarAction};
use crate::log::reproducibility::ReproLog;
use crate::pipeline::conversion;
use crate::pipeline::processing::{self, ProcessingOp};

/// Which domain tab the user is viewing
#[derive(Clone, Copy, PartialEq)]
enum DomainTab {
    TimeDomain,
    FrequencyDomain,
    Export,
}

/// Serializable project state for save/load
#[derive(serde::Serialize, serde::Deserialize)]
struct ProjectSave {
    spectrum: Option<SpectrumData>,
    fid_snapshot: Option<SpectrumData>,
    is_frequency_domain: bool,
    // Annotations
    peaks: Vec<[f64; 2]>,
    multiplets: Vec<crate::pipeline::processing::Multiplet>,
    integrations: Vec<(f64, f64, f64)>,
    integration_reference_h: f64,
    j_couplings: Vec<(f64, f64, f64, f64)>,
    baseline_points: Vec<[f64; 2]>,
    // Metadata
    theme: String,
    sample_name: String,
}

/// The main application
pub struct NmrApp {
    /// Currently loaded / working spectrum
    spectrum: Option<SpectrumData>,

    /// Snapshot of the FID right before FT was applied, so the user
    /// can flip back and inspect the time-domain data.
    fid_snapshot: Option<SpectrumData>,
    /// Which domain tab is selected
    domain_tab: DomainTab,

    /// Undo history: stack of (operation, snapshot-before)
    undo_stack: Vec<(ProcessingOp, SpectrumData)>,
    /// Redo stack
    redo_stack: Vec<(ProcessingOp, SpectrumData)>,

    /// "Before" spectrum for comparison
    before_snapshot: Option<SpectrumData>,

    /// Reproducibility log
    repro_log: ReproLog,

    /// GUI sub-states
    pipeline_state: PipelinePanelState,
    spectrum_view_state: SpectrumViewState,
    contour_view_state: ContourViewState,
    phase_dialog_state: PhaseDialogState,
    conversion_dialog_state: ConversionDialogState,
    export_dialog_state: ExportDialogState,
    export_tab_state: ExportTabState,

    /// Status messages
    status_message: String,
    show_log_window: bool,
    show_about: bool,

    /// NMRPipe availability
    nmrpipe_available: bool,

    /// Current theme
    current_theme: AppTheme,
    theme_colors: ThemeColors,

    /// Current conversion method (NMRPipe vs Built-in)
    conversion_method: crate::gui::conversion_dialog::ConversionMethod,

    /// Dropped files buffer
    dropped_files: Vec<PathBuf>,
}

impl NmrApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // ── Apply default theme ──
        let default_theme = AppTheme::Light;
        theme::apply_theme(&cc.egui_ctx, default_theme);
        let theme_colors = ThemeColors::from_theme(default_theme);

        // ── Typography: scale for monitor DPI ──
        let ppi = cc.egui_ctx.pixels_per_point();
        let base_size = if ppi > 1.5 { 14.0 } else { 13.0 };
        let mut style = (*cc.egui_ctx.style()).clone();
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(base_size, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(base_size, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(base_size * 1.25, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::new(base_size * 0.85, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::new(base_size * 0.92, egui::FontFamily::Monospace),
        );
        // More breathing room in panels
        style.spacing.item_spacing = egui::vec2(8.0, 5.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.spacing.indent = 18.0;
        cc.egui_ctx.set_style(style);

        let nmrpipe_available = crate::pipeline::command::check_nmrpipe_available();
        if nmrpipe_available {
            log::info!("NMRPipe detected on system");
        } else {
            log::info!("NMRPipe not found — using built-in processing");
        }

        Self {
            spectrum: None,
            fid_snapshot: None,
            domain_tab: DomainTab::TimeDomain,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            before_snapshot: None,
            repro_log: ReproLog::new(),
            pipeline_state: PipelinePanelState::default(),
            spectrum_view_state: SpectrumViewState::default(),
            contour_view_state: ContourViewState::default(),
            phase_dialog_state: PhaseDialogState::default(),
            conversion_dialog_state: ConversionDialogState::default(),
            export_dialog_state: ExportDialogState::default(),
            export_tab_state: ExportTabState::default(),
            status_message: "Ready — open an NMR data file or folder to begin".to_string(),
            show_log_window: false,
            show_about: false,
            nmrpipe_available,
            current_theme: default_theme,
            theme_colors: theme_colors,
            conversion_method: if nmrpipe_available {
                crate::gui::conversion_dialog::ConversionMethod::NMRPipe
            } else {
                crate::gui::conversion_dialog::ConversionMethod::BuiltIn
            },
            dropped_files: Vec::new(),
        }
    }

    /// Load a file or folder.
    /// For JDF files, opens the conversion dialog first so the user can set parameters.
    fn load_path(&mut self, path: PathBuf) {
        // If it's a directory, find NMR files in it
        let files_to_try = if path.is_dir() {
            let files = conversion::list_nmr_files(&path);
            if files.is_empty() {
                self.status_message = format!("No NMR data files found in: {}", path.display());
                return;
            }
            files
        } else {
            vec![path.clone()]
        };

        let target = files_to_try[0].clone();
        let format = conversion::detect_format(&target);

        // For JEOL files, show the conversion settings dialog
        if format == crate::data::spectrum::VendorFormat::Jeol {
            self.conversion_dialog_state.open = true;
            self.conversion_dialog_state.pending_path = Some(target);
            self.conversion_dialog_state.info_loaded = false;
            self.conversion_dialog_state.info_text.clear();
            // Keep existing settings so user adjustments persist between loads
            self.status_message = "Configure delta2pipe settings, then click Convert…".to_string();
            return;
        }

        // Non-JDF: load directly
        self.do_load(&target, None);
    }

    /// Build ConversionSettings with the current conversion method
    fn make_settings(&self, base: Option<&crate::gui::conversion_dialog::ConversionSettings>) -> crate::gui::conversion_dialog::ConversionSettings {
        let mut s = base.cloned().unwrap_or_default();
        s.conversion_method = self.conversion_method;
        s
    }

    /// Actually perform the loading (after any dialog).
    fn do_load(
        &mut self,
        path: &std::path::Path,
        settings: Option<&crate::gui::conversion_dialog::ConversionSettings>,
    ) {
        self.status_message = format!("Loading: {}…", path.display());
        self.repro_log = ReproLog::new();
        self.repro_log.set_source(&path.to_string_lossy());
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.before_snapshot = None;
        self.fid_snapshot = None;
        // Reset phase dialog from previous file
        self.phase_dialog_state = PhaseDialogState::default();

        // Reset all annotations from previous file
        self.spectrum_view_state.peaks.clear();
        self.spectrum_view_state.multiplets.clear();
        self.spectrum_view_state.integrations.clear();
        self.spectrum_view_state.integration_start = None;
        self.spectrum_view_state.j_couplings.clear();
        self.spectrum_view_state.j_coupling_first = None;
        self.spectrum_view_state.baseline_points.clear();
        self.spectrum_view_state.peak_picking = false;
        self.spectrum_view_state.baseline_picking = false;
        self.spectrum_view_state.integration_picking = false;
        self.spectrum_view_state.j_coupling_picking = false;
        self.spectrum_view_state.auto_scale = true;

        // Merge user-provided settings with current conversion method
        let merged = self.make_settings(settings);

        // Set domain tab based on what we actually loaded
        // (will be updated below after successful load to match the data)

        match conversion::load_spectrum(path, &mut self.repro_log, Some(&merged)) {
            Ok(spectrum) => {
                // Auto-select the correct domain tab based on loaded data
                if spectrum.is_frequency_domain {
                    self.domain_tab = DomainTab::FrequencyDomain;
                } else {
                    self.domain_tab = DomainTab::TimeDomain;
                }
                let pts_info = if spectrum.is_2d() {
                    format!("{}×{}",
                        spectrum.data_2d.len(),
                        spectrum.data_2d.first().map(|r| r.len()).unwrap_or(0))
                } else {
                    format!("{} pts", spectrum.real.len())
                };
                self.status_message = format!(
                    "Loaded: {} ({}, {}, {}) [{}]",
                    spectrum.sample_name,
                    spectrum.experiment_type,
                    pts_info,
                    spectrum.vendor_format,
                    if spectrum.conversion_method_used.is_empty() {
                        "unknown method"
                    } else {
                        &spectrum.conversion_method_used
                    },
                );
                // Set nucleus and experiment info in the log
                let nucleus = spectrum.axes.first()
                    .map(|a| a.nucleus.to_string())
                    .unwrap_or_default();
                self.repro_log.set_spectrum_info(&nucleus, &spectrum.experiment_type.to_string());
                self.spectrum = Some(spectrum);
            }
            Err(e) => {
                self.status_message = format!("Error loading {}: {}", path.display(), e);
                log::error!("Load error: {}", e);
            }
        }
    }

    /// Save a snapshot before an operation (for undo)
    fn push_undo(&mut self, op: ProcessingOp) {
        if let Some(spectrum) = &self.spectrum {
            self.before_snapshot = Some(spectrum.clone());
            self.undo_stack.push((op, spectrum.clone()));
            self.redo_stack.clear(); // Clear redo on new action
        }
    }

    /// Undo the last operation
    fn undo(&mut self) {
        if let Some((op, snapshot)) = self.undo_stack.pop() {
            if let Some(current) = self.spectrum.take() {
                self.redo_stack.push((op.clone(), current));
            }
            self.spectrum = Some(snapshot);
            self.before_snapshot = None; // Clear stale comparison
            self.repro_log.pop_entry();
            self.status_message = format!("Undone: {}", op);
        }
    }

    /// Redo the last undone operation
    fn redo(&mut self) {
        if let Some((op, snapshot)) = self.redo_stack.pop() {
            if let Some(current) = self.spectrum.take() {
                self.undo_stack.push((op.clone(), current));
            }
            self.spectrum = Some(snapshot);
            self.status_message = format!("Redone: {}", op);
        }
    }

    /// Export the current spectrum to a PNG or SVG image file with configurable settings.
    fn export_spectrum_image_with_settings(
        &self,
        path: &std::path::Path,
        settings: &ExportSettings,
    ) -> Result<(), String> {
        let spectrum = self.spectrum.as_ref().ok_or("No spectrum loaded")?;
        if spectrum.real.is_empty() {
            return Err("Spectrum has no data".to_string());
        }

        let width = settings.width;
        let height = settings.height;
        let margin_left: u32 = (width as f64 * 0.04).max(80.0) as u32;
        let margin_right: u32 = (width as f64 * 0.025).max(40.0) as u32;
        let margin_top: u32 = (height as f64 * 0.08).max(50.0) as u32;
        let margin_bottom: u32 = (height as f64 * 0.10).max(70.0) as u32;
        let plot_w = width - margin_left - margin_right;
        let plot_h = height - margin_top - margin_bottom;

        // Build ppm scale
        let ppm_scale = if spectrum.is_frequency_domain && !spectrum.axes.is_empty() {
            spectrum.axes[0].ppm_scale()
        } else {
            (0..spectrum.real.len()).map(|i| i as f64).collect::<Vec<_>>()
        };

        // Determine x range (ppm) — user-configurable
        let (ppm_hi, ppm_lo) = if settings.use_custom_range {
            (
                settings.ppm_start.max(settings.ppm_end),
                settings.ppm_start.min(settings.ppm_end),
            )
        } else {
            let ppm_min = ppm_scale.iter().cloned().fold(f64::INFINITY, f64::min);
            let ppm_max = ppm_scale.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            (ppm_max, ppm_min)
        };
        let x_range = ppm_hi - ppm_lo;
        if x_range <= 0.0 {
            return Err("Invalid ppm range".to_string());
        }

        // Filter data to ppm range
        let clip_neg = settings.clip_negatives;
        let y_data: Vec<(f64, f64)> = ppm_scale
            .iter()
            .zip(spectrum.real.iter())
            .filter(|(&ppm, _)| ppm >= ppm_lo && ppm <= ppm_hi)
            .map(|(&ppm, &y)| (ppm, if clip_neg { y.max(0.0) } else { y }))
            .collect();

        if y_data.is_empty() {
            return Err("No data points in the selected PPM range".to_string());
        }

        let y_min = if clip_neg {
            0.0
        } else {
            y_data.iter().map(|d| d.1).fold(f64::INFINITY, f64::min)
        };
        let y_max = y_data.iter().map(|d| d.1).fold(f64::NEG_INFINITY, f64::max);
        let y_range = (y_max - y_min).max(1e-12);
        // Add 5% padding at top
        let y_max_padded = y_max + y_range * 0.05;
        let y_range_padded = (y_max_padded - y_min).max(1e-12);

        // Title
        let title = if settings.use_custom_title && !settings.custom_title.is_empty() {
            settings.custom_title.clone()
        } else {
            format!(
                "{} — {} — {} pts",
                spectrum.sample_name,
                spectrum.experiment_type,
                spectrum.real.len()
            )
        };

        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        match ext.as_str() {
            "svg" => self.export_svg(
                path, spectrum, &ppm_scale, &title,
                ppm_hi, ppm_lo, x_range, y_min, y_max_padded, y_range_padded,
                clip_neg, settings, width, height,
                margin_left, margin_right, margin_top, margin_bottom, plot_w, plot_h,
            ),
            _ => self.export_png(
                path, spectrum, &ppm_scale, &title,
                ppm_hi, ppm_lo, x_range, y_min, y_max_padded, y_range_padded,
                clip_neg, settings, width, height,
                margin_left, margin_right, margin_top, margin_bottom, plot_w, plot_h,
            ),
        }
    }

    fn export_png(
        &self,
        path: &std::path::Path,
        spectrum: &SpectrumData,
        ppm_scale: &[f64],
        title: &str,
        ppm_hi: f64, ppm_lo: f64, x_range: f64,
        y_min: f64, _y_max: f64, y_range: f64,
        clip_neg: bool,
        settings: &ExportSettings,
        width: u32, height: u32,
        margin_left: u32, _margin_right: u32, margin_top: u32, _margin_bottom: u32,
        plot_w: u32, plot_h: u32,
    ) -> Result<(), String> {
        let mut imgbuf = image::RgbImage::from_pixel(width, height, image::Rgb([255, 255, 255]));

        // Scale factors
        let ms_f = settings.marker_scale;
        let fs_f = settings.font_scale;
        let ts = (2.0 * fs_f).round().max(1.0) as u32;           // text scale for labels
        let title_ts = (3.0 * fs_f).round().max(1.0) as u32;     // text scale for title
        let char_w = (4 * ts) as i32;                             // char width in px
        let char_h = (5 * ts) as i32;                             // char height in px
        let marker_size = (4.0 * ms_f).round().max(1.0) as i32;  // triangle half-width
        let marker_gap = (6.0 * ms_f).round().max(2.0) as i32;   // gap between peak and marker

        // Draw grid lines
        if settings.show_grid {
            let grid_color = image::Rgb([230, 230, 235]);
            let num_grid_x = ((x_range / 1.0).ceil() as u32).min(20).max(5);
            for gi in 1..num_grid_x {
                let gx = margin_left + (plot_w as f64 * gi as f64 / num_grid_x as f64) as u32;
                for y in margin_top + 1..margin_top + plot_h {
                    if gx < width { imgbuf.put_pixel(gx, y, grid_color); }
                }
            }
            for gi in 1..5 {
                let gy = margin_top + (plot_h as f64 * gi as f64 / 5.0) as u32;
                for x in margin_left + 1..margin_left + plot_w {
                    if gy < height { imgbuf.put_pixel(x, gy, grid_color); }
                }
            }
        }

        // Draw plot border
        let border_color = image::Rgb([100, 100, 110]);
        for x in margin_left..=margin_left + plot_w {
            if x < width {
                imgbuf.put_pixel(x, margin_top, border_color);
                imgbuf.put_pixel(x, margin_top + plot_h, border_color);
            }
        }
        for y in margin_top..=margin_top + plot_h {
            if y < height {
                imgbuf.put_pixel(margin_left, y, border_color);
                imgbuf.put_pixel(margin_left + plot_w, y, border_color);
            }
        }

        // Draw spectrum — NMR convention: high ppm on left
        let spec_color = image::Rgb([26, 58, 107]); // dark navy
        let n = spectrum.real.len().min(ppm_scale.len());
        let mut prev_px: Option<(i32, i32)> = None;
        for i in 0..n {
            let ppm = ppm_scale[i];
            if ppm < ppm_lo || ppm > ppm_hi {
                prev_px = None;
                continue;
            }
            let x_frac = (ppm_hi - ppm) / x_range;
            let px_x = margin_left as i32 + (x_frac * plot_w as f64) as i32;
            let y_val = if clip_neg { spectrum.real[i].max(0.0) } else { spectrum.real[i] };
            let y_frac = 1.0 - (y_val - y_min) / y_range;
            let px_y = margin_top as i32 + (y_frac * plot_h as f64).clamp(0.0, plot_h as f64) as i32;

            if let Some((px, py)) = prev_px {
                draw_line(&mut imgbuf, px, py, px_x, px_y, spec_color, width, height);
            }
            prev_px = Some((px_x, px_y));
        }

        // Draw peak markers with collision-avoidant labels
        if settings.show_peaks {
            let peak_color = image::Rgb([224, 48, 48]);
            let leader_color = image::Rgb([200, 120, 120]);

            // Phase 1: Collect all visible peak positions and label info
            struct PeakLabel {
                px_x: i32,
                px_y: i32,
                label: String,
                label_x: i32,
                label_y: i32,
                label_w: i32,
                label_h: i32,
            }
            let mut labels: Vec<PeakLabel> = Vec::new();
            let label_pad = (char_h / 2).max(4);   // padding between labels

            for peak in &self.spectrum_view_state.peaks {
                if peak[0] < ppm_lo || peak[0] > ppm_hi { continue; }
                let x_frac = (ppm_hi - peak[0]) / x_range;
                let px_x = margin_left as i32 + (x_frac * plot_w as f64) as i32;
                let y_val = if clip_neg { peak[1].max(0.0) } else { peak[1] };
                let y_frac = 1.0 - (y_val - y_min) / y_range;
                let px_y = margin_top as i32 + (y_frac * plot_h as f64).clamp(0.0, plot_h as f64) as i32;

                let label = format!("{:.2}", peak[0]);
                let label_w = label.len() as i32 * char_w;
                let label_h = char_h;
                let label_x = px_x - label_w / 2;
                // Natural position: above the marker triangle
                let label_y = px_y - marker_gap - marker_size - label_pad - label_h;

                labels.push(PeakLabel {
                    px_x, px_y, label, label_x, label_y, label_w, label_h,
                });
            }

            // Phase 2: Collision avoidance — multi-pass, check all pairs
            labels.sort_by_key(|l| l.label_x);
            for _pass in 0..5 {
                let mut any_moved = false;
                for i in 0..labels.len() {
                    for _iter in 0..20 {
                        let mut needs_shift = false;
                        let mut shift_to = 0i32;
                        for j in 0..labels.len() {
                            if j == i { continue; }
                            let (ax, ay, aw, ah) = (labels[i].label_x, labels[i].label_y, labels[i].label_w, labels[i].label_h);
                            let (bx, by, bw, bh) = (labels[j].label_x, labels[j].label_y, labels[j].label_w, labels[j].label_h);
                            // AABB overlap check with padding
                            if ax < bx + bw + label_pad && bx < ax + aw + label_pad
                                && ay < by + bh + label_pad && by < ay + ah + label_pad
                            {
                                let target = labels[j].label_y - labels[i].label_h - label_pad;
                                if !needs_shift || target < shift_to {
                                    shift_to = target;
                                }
                                needs_shift = true;
                            }
                        }
                        if needs_shift {
                            // Don't shift above the title area
                            let min_y = (15 + char_h * 2) as i32;
                            labels[i].label_y = shift_to.max(min_y);
                            any_moved = true;
                        } else {
                            break;
                        }
                    }
                }
                if !any_moved { break; }
            }

            // Phase 3: Draw markers, leader lines, and labels
            for pl in &labels {
                // Triangle marker (pointing down at peak)
                for dx in -marker_size..=marker_size {
                    for dy in 0..=marker_size {
                        if dx.abs() <= dy {
                            let mx = pl.px_x + dx;
                            let my = pl.px_y - marker_gap - dy;
                            if mx >= 0 && mx < width as i32 && my >= 0 && my < height as i32 {
                                imgbuf.put_pixel(mx as u32, my as u32, peak_color);
                            }
                        }
                    }
                }

                // Leader line if label was displaced from natural position
                let natural_y = pl.px_y - marker_gap - marker_size - label_pad - pl.label_h;
                if pl.label_y < natural_y - label_pad {
                    let line_x = pl.px_x;
                    let line_top = pl.label_y + pl.label_h + 1;
                    let line_bot = pl.px_y - marker_gap - marker_size;
                    if line_top < line_bot {
                        for ly in line_top..line_bot {
                            if line_x >= 0 && line_x < width as i32 && ly >= 0 && ly < height as i32 {
                                imgbuf.put_pixel(line_x as u32, ly as u32, leader_color);
                            }
                        }
                    }
                }

                // Draw label text
                draw_simple_text(
                    &mut imgbuf,
                    &pl.label,
                    pl.label_x.max(0) as u32,
                    pl.label_y.max(0) as u32,
                    peak_color,
                    ts,
                );
            }
        }

        // ── Below-plot stacked labels ──
        let tick_step = smart_tick_step(x_range);
        let first_tick = (ppm_lo / tick_step).ceil() * tick_step;
        let tick_len = (4.0 * ms_f).round().max(2.0) as u32;
        let row_gap = (4.0 * fs_f).round().max(3.0) as u32;
        let char_h_u = char_h as u32;

        // Row 1: tick marks + axis labels
        let tick_label_y = margin_top + plot_h + tick_len + row_gap;
        {
            let mut tick = first_tick;
            while tick <= ppm_hi {
                let x_frac = (ppm_hi - tick) / x_range;
                let gx = margin_left + (x_frac * plot_w as f64) as u32;
                let label = format!("{:.1}", tick);
                let label_w = label.len() as u32 * (4 * ts);
                draw_simple_text(
                    &mut imgbuf,
                    &label,
                    gx.saturating_sub(label_w / 2),
                    tick_label_y,
                    image::Rgb([60, 60, 70]),
                    ts,
                );
                // Tick mark
                for dy in 0..tick_len {
                    if gx < width && margin_top + plot_h + dy < height {
                        imgbuf.put_pixel(gx, margin_top + plot_h + dy, image::Rgb([100, 100, 110]));
                    }
                }
                tick += tick_step;
            }
        }
        let mut next_row_y = tick_label_y + char_h_u + row_gap;

        // Row 2: Integration labels
        if settings.show_integrations && !self.spectrum_view_state.integrations.is_empty() {
            let int_color = image::Rgb([76, 175, 80]);
            let first_raw = self.spectrum_view_state.integrations
                .first()
                .map(|r| r.2)
                .unwrap_or(1.0)
                .abs()
                .max(1e-12);

            for &(start_ppm, end_ppm, raw_val) in &self.spectrum_view_state.integrations {
                let lo = start_ppm.min(end_ppm).max(ppm_lo);
                let hi = start_ppm.max(end_ppm).min(ppm_hi);
                if lo >= hi { continue; }

                // Draw dashed boundary lines
                let x_lo = margin_left as i32 + ((ppm_hi - hi) / x_range * plot_w as f64) as i32;
                let x_hi = margin_left as i32 + ((ppm_hi - lo) / x_range * plot_w as f64) as i32;
                let dash_len = (4.0 * ms_f).round().max(2.0) as u32;
                let gap_len = (2.0 * ms_f).round().max(1.0) as u32;
                for y in margin_top..margin_top + plot_h {
                    let cycle = (y - margin_top) % (dash_len + gap_len);
                    if cycle < dash_len {
                        if x_lo >= 0 && x_lo < width as i32 {
                            imgbuf.put_pixel(x_lo as u32, y, int_color);
                        }
                        if x_hi >= 0 && x_hi < width as i32 {
                            imgbuf.put_pixel(x_hi as u32, y, int_color);
                        }
                    }
                }
                // Integral value label
                let mid_x = (x_lo + x_hi) / 2;
                let rel_val = raw_val / first_raw;
                let h_val = rel_val * self.spectrum_view_state.integration_reference_h;
                let label = format!("{:.2}H", h_val);
                let label_w = label.len() as i32 * char_w;
                let label_x = (mid_x - label_w / 2).max(0) as u32;
                draw_simple_text(&mut imgbuf, &label, label_x, next_row_y, int_color, ts);
            }
            next_row_y += char_h_u + row_gap;
        }

        // Row 3: Multiplet labels
        if settings.show_multiplets && !self.spectrum_view_state.multiplets.is_empty() {
            let mult_color = image::Rgb([0, 96, 170]);
            for mult in &self.spectrum_view_state.multiplets {
                if mult.center_ppm < ppm_lo || mult.center_ppm > ppm_hi { continue; }
                let x_frac = (ppm_hi - mult.center_ppm) / x_range;
                let px_x = margin_left as i32 + (x_frac * plot_w as f64) as i32;
                let label = if mult.j_hz > 0.5 {
                    format!("{} J={:.1}", mult.label, mult.j_hz)
                } else {
                    mult.label.clone()
                };
                let label_w = label.len() as i32 * char_w;
                let label_x = (px_x - label_w / 2).max(0) as u32;
                draw_simple_text(&mut imgbuf, &label, label_x, next_row_y, mult_color, ts);
            }
            next_row_y += char_h_u + row_gap;
        }

        // X-axis title
        let axis_title = "Chemical Shift (ppm)";
        let axis_title_w = axis_title.len() as u32 * (4 * ts);
        draw_simple_text(
            &mut imgbuf,
            axis_title,
            margin_left + plot_w / 2 - axis_title_w / 2,
            next_row_y + row_gap,
            image::Rgb([60, 60, 70]),
            ts,
        );

        // Draw title
        draw_simple_text(&mut imgbuf, title, margin_left, 15, image::Rgb([40, 40, 50]), title_ts);

        // PPM range info
        let range_info = format!("{:.1} - {:.1} ppm", ppm_hi, ppm_lo);
        let range_w = range_info.len() as u32 * (4 * ts);
        draw_simple_text(
            &mut imgbuf,
            &range_info,
            (margin_left + plot_w).saturating_sub(range_w),
            15,
            image::Rgb([120, 120, 130]),
            ts,
        );

        imgbuf.save(path).map_err(|e| e.to_string())
    }

    fn export_svg(
        &self,
        path: &std::path::Path,
        spectrum: &SpectrumData,
        ppm_scale: &[f64],
        title: &str,
        ppm_hi: f64, ppm_lo: f64, x_range: f64,
        y_min: f64, _y_max: f64, y_range: f64,
        clip_neg: bool,
        settings: &ExportSettings,
        width: u32, height: u32,
        margin_left: u32, _margin_right: u32, margin_top: u32, _margin_bottom: u32,
        plot_w: u32, plot_h: u32,
    ) -> Result<(), String> {
        let mut svg = String::new();
        svg.push_str(&format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='{}' height='{}'>\n",
            width, height
        ));
        svg.push_str("<rect width='100%' height='100%' fill='white'/>\n");

        // Scale factors
        let ms = settings.marker_scale as f64;
        let fs = settings.font_scale as f64;
        let font_sm = (10.0 * fs).round().max(6.0);   // small label font
        let font_md = (12.0 * fs).round().max(7.0);   // axis label font
        let font_lg = (16.0 * fs).round().max(8.0);   // title font
        let font_ax = (13.0 * fs).round().max(7.0);   // axis title font
        let font_rng = (11.0 * fs).round().max(6.0);  // range info font
        let marker_h = 8.0 * ms;                       // marker bottom-to-tip
        let marker_w = 4.0 * ms;                       // marker half-width

        // Grid lines
        if settings.show_grid {
            let tick_step = smart_tick_step(x_range);
            let first_tick = (ppm_lo / tick_step).ceil() * tick_step;
            let mut tick = first_tick;
            while tick <= ppm_hi {
                let x_frac = (ppm_hi - tick) / x_range;
                let sx = margin_left as f64 + x_frac * plot_w as f64;
                svg.push_str(&format!(
                    "<line x1='{:.1}' y1='{}' x2='{:.1}' y2='{}' stroke='#E6E6EB' stroke-width='0.5'/>\n",
                    sx, margin_top, sx, margin_top + plot_h
                ));
                tick += tick_step;
            }
        }

        // Spectrum polyline
        let n = spectrum.real.len().min(ppm_scale.len());
        svg.push_str(&format!(
            "<polyline fill='none' stroke='#1A3A6B' stroke-width='{:.1}' points='",
            settings.line_width
        ));
        for i in 0..n {
            let ppm = ppm_scale[i];
            if ppm < ppm_lo || ppm > ppm_hi { continue; }
            let x_frac = (ppm_hi - ppm) / x_range;
            let sx = margin_left as f64 + x_frac * plot_w as f64;
            let y_val = if clip_neg { spectrum.real[i].max(0.0) } else { spectrum.real[i] };
            let y_frac = 1.0 - (y_val - y_min) / y_range;
            let sy = margin_top as f64 + (y_frac * plot_h as f64).clamp(0.0, plot_h as f64);
            svg.push_str(&format!("{:.1},{:.1} ", sx, sy));
        }
        svg.push_str("'/>\n");

        // Plot border
        svg.push_str(&format!(
            "<rect x='{}' y='{}' width='{}' height='{}' fill='none' stroke='#64646E' stroke-width='1'/>\n",
            margin_left, margin_top, plot_w, plot_h
        ));

        // Peak markers with collision-avoidant labels
        if settings.show_peaks {
            // Collect peak positions and labels
            struct SvgPeakLabel {
                sx: f64,
                sy: f64,
                label: String,
                label_x: f64,
                label_y: f64,
                label_w: f64,
                label_h: f64,
            }
            let mut labels: Vec<SvgPeakLabel> = Vec::new();
            let char_w_est = font_sm * 0.6; // approximate char width for font
            let label_pad = font_sm * 0.4;  // padding between labels

            for peak in &self.spectrum_view_state.peaks {
                if peak[0] < ppm_lo || peak[0] > ppm_hi { continue; }
                let x_frac = (ppm_hi - peak[0]) / x_range;
                let sx = margin_left as f64 + x_frac * plot_w as f64;
                let y_val = if clip_neg { peak[1].max(0.0) } else { peak[1] };
                let y_frac = 1.0 - (y_val - y_min) / y_range;
                let sy = margin_top as f64 + (y_frac * plot_h as f64).clamp(0.0, plot_h as f64);

                let label = format!("{:.2}", peak[0]);
                let label_w = label.len() as f64 * char_w_est;
                let label_h = font_sm * 1.2;
                let label_x = sx - label_w / 2.0;
                let label_y = sy - marker_h * 2.5 - label_h - label_pad;

                labels.push(SvgPeakLabel {
                    sx, sy, label, label_x, label_y, label_w, label_h,
                });
            }

            // Collision avoidance — multi-pass, check all pairs
            labels.sort_by(|a, b| a.label_x.partial_cmp(&b.label_x).unwrap_or(std::cmp::Ordering::Equal));
            for _pass in 0..5 {
                let mut any_moved = false;
                for i in 0..labels.len() {
                    for _iter in 0..20 {
                        let mut needs_shift = false;
                        let mut shift_to = 0.0f64;
                        for j in 0..labels.len() {
                            if j == i { continue; }
                            let (ax, ay, aw, ah) = (labels[i].label_x, labels[i].label_y, labels[i].label_w, labels[i].label_h);
                            let (bx, by, bw, bh) = (labels[j].label_x, labels[j].label_y, labels[j].label_w, labels[j].label_h);
                            if ax < bx + bw + label_pad && bx < ax + aw + label_pad
                                && ay < by + bh + label_pad && by < ay + ah + label_pad
                            {
                                let target = labels[j].label_y - labels[i].label_h - label_pad;
                                if !needs_shift || target < shift_to {
                                    shift_to = target;
                                }
                                needs_shift = true;
                            }
                        }
                        if needs_shift {
                            let min_y = 30.0 + font_lg + 4.0;
                            labels[i].label_y = shift_to.max(min_y);
                            any_moved = true;
                        } else {
                            break;
                        }
                    }
                }
                if !any_moved { break; }
            }

            // Draw markers, leader lines, labels
            for pl in &labels {
                // Triangle marker
                svg.push_str(&format!(
                    "<polygon points='{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}' fill='#E03030'/>\n",
                    pl.sx, pl.sy - marker_h,
                    pl.sx - marker_w, pl.sy - marker_h * 2.5,
                    pl.sx + marker_w, pl.sy - marker_h * 2.5
                ));

                // Leader line if displaced
                let natural_y = pl.sy - marker_h * 2.5 - pl.label_h - label_pad;
                if pl.label_y < natural_y - label_pad {
                    svg.push_str(&format!(
                        "<line x1='{:.1}' y1='{:.1}' x2='{:.1}' y2='{:.1}' stroke='#C87878' stroke-width='0.5'/>\n",
                        pl.sx, pl.label_y + pl.label_h,
                        pl.sx, pl.sy - marker_h * 2.5
                    ));
                }

                // Label
                svg.push_str(&format!(
                    "<text x='{:.0}' y='{:.0}' font-family='sans-serif' font-size='{:.0}' fill='#C02020' text-anchor='middle'>{}</text>\n",
                    pl.sx, pl.label_y + pl.label_h * 0.8, font_sm, pl.label
                ));
            }
        }

        // ── Below-plot stacked labels ──
        let tick_step = smart_tick_step(x_range);
        let first_tick = (ppm_lo / tick_step).ceil() * tick_step;
        let row_gap = (4.0 * fs).max(3.0);

        // Row 1: tick marks + axis labels
        let tick_label_y = margin_top as f64 + plot_h as f64 + 6.0 + font_md;
        {
            let mut tick = first_tick;
            while tick <= ppm_hi {
                let x_frac = (ppm_hi - tick) / x_range;
                let gx = margin_left as f64 + plot_w as f64 * x_frac;
                svg.push_str(&format!(
                    "<text x='{:.0}' y='{:.0}' font-family='sans-serif' font-size='{:.0}' fill='#3C3C46' text-anchor='middle'>{:.1}</text>\n",
                    gx, tick_label_y, font_md, tick
                ));
                tick += tick_step;
            }
        }
        let mut next_row_y = tick_label_y + row_gap;

        // Row 2: Integration labels
        if settings.show_integrations && !self.spectrum_view_state.integrations.is_empty() {
            let first_raw = self.spectrum_view_state.integrations
                .first()
                .map(|r| r.2)
                .unwrap_or(1.0)
                .abs()
                .max(1e-12);
            for &(start_ppm, end_ppm, raw_val) in &self.spectrum_view_state.integrations {
                let lo = start_ppm.min(end_ppm).max(ppm_lo);
                let hi = start_ppm.max(end_ppm).min(ppm_hi);
                if lo >= hi { continue; }
                let x_lo = margin_left as f64 + (ppm_hi - hi) / x_range * plot_w as f64;
                let x_hi = margin_left as f64 + (ppm_hi - lo) / x_range * plot_w as f64;
                svg.push_str(&format!(
                    "<line x1='{:.1}' y1='{}' x2='{:.1}' y2='{}' stroke='#4CAF50' stroke-width='1' stroke-dasharray='4,2'/>\n",
                    x_lo, margin_top, x_lo, margin_top + plot_h
                ));
                svg.push_str(&format!(
                    "<line x1='{:.1}' y1='{}' x2='{:.1}' y2='{}' stroke='#4CAF50' stroke-width='1' stroke-dasharray='4,2'/>\n",
                    x_hi, margin_top, x_hi, margin_top + plot_h
                ));
                let mid_x = (x_lo + x_hi) / 2.0;
                let rel_val = raw_val / first_raw;
                let h_val = rel_val * self.spectrum_view_state.integration_reference_h;
                svg.push_str(&format!(
                    "<text x='{:.0}' y='{:.0}' font-family='sans-serif' font-size='{:.0}' fill='#4CAF50' text-anchor='middle'>{:.2}H</text>\n",
                    mid_x, next_row_y + font_sm, font_sm + 1.0, h_val
                ));
            }
            next_row_y += font_sm + row_gap;
        }

        // Row 3: Multiplet labels
        if settings.show_multiplets && !self.spectrum_view_state.multiplets.is_empty() {
            for mult in &self.spectrum_view_state.multiplets {
                if mult.center_ppm < ppm_lo || mult.center_ppm > ppm_hi { continue; }
                let x_frac = (ppm_hi - mult.center_ppm) / x_range;
                let sx = margin_left as f64 + x_frac * plot_w as f64;
                let label = if mult.j_hz > 0.5 {
                    format!("{}, J={:.1}", mult.label, mult.j_hz)
                } else {
                    mult.label.clone()
                };
                svg.push_str(&format!(
                    "<text x='{:.0}' y='{:.0}' font-family='sans-serif' font-size='{:.0}' fill='#0060AA' text-anchor='middle'>{}</text>\n",
                    sx, next_row_y + font_sm, font_sm, label
                ));
            }
            next_row_y += font_sm + row_gap;
        }

        // X-axis title
        svg.push_str(&format!(
            "<text x='{}' y='{:.0}' font-family='sans-serif' font-size='{:.0}' fill='#3C3C46' text-anchor='middle'>Chemical Shift (ppm)</text>\n",
            margin_left + plot_w / 2,
            next_row_y + font_ax + row_gap,
            font_ax
        ));

        // Title
        svg.push_str(&format!(
            "<text x='{}' y='30' font-family='sans-serif' font-size='{:.0}' fill='#282832'>{}</text>\n",
            margin_left,
            font_lg,
            title.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        ));

        // PPM range annotation
        svg.push_str(&format!(
            "<text x='{}' y='30' font-family='sans-serif' font-size='{:.0}' fill='#78787E' text-anchor='end'>{:.1} – {:.1} ppm</text>\n",
            margin_left + plot_w, font_rng, ppm_hi, ppm_lo
        ));

        svg.push_str("</svg>\n");
        std::fs::write(path, svg).map_err(|e| e.to_string())
    }

    /// Export peak list, integration, multiplet, and J-coupling data to CSV/TSV/TXT.
    fn export_data_report(&self, path: &std::path::Path) -> Result<(), String> {
        let spectrum = self.spectrum.as_ref().ok_or("No spectrum loaded")?;

        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let sep = match ext.as_str() {
            "csv" => ",",
            "tsv" => "\t",
            _ => "\t",
        };

        let mut out = String::new();

        // ── Header ──
        out.push_str(&format!(
            "# NMR Data Report{}\n",
            if ext == "csv" { "" } else { "" }
        ));
        out.push_str(&format!("# Sample: {}\n", spectrum.sample_name));
        out.push_str(&format!("# Experiment: {}\n", spectrum.experiment_type));
        out.push_str(&format!("# Data points: {}\n", spectrum.real.len()));
        if !spectrum.axes.is_empty() {
            let ax = &spectrum.axes[0];
            out.push_str(&format!(
                "# Observe freq: {:.4} MHz\n",
                ax.observe_freq_mhz
            ));
            out.push_str(&format!(
                "# Spectral width: {:.2} Hz ({:.4} ppm)\n",
                ax.spectral_width_hz,
                ax.spectral_width_hz / ax.observe_freq_mhz
            ));
            out.push_str(&format!("# Nucleus: {}\n", ax.nucleus));
        }
        out.push_str(&format!(
            "# Generated: {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        ));
        out.push('\n');

        // ── Peak List ──
        let peaks = &self.spectrum_view_state.peaks;
        if !peaks.is_empty() {
            out.push_str(&format!(
                "# Peak List ({} peaks)\n",
                peaks.len()
            ));
            out.push_str(&format!(
                "Peak_No{}Chemical_Shift_ppm{}Intensity{}Relative_Intensity\n",
                sep, sep, sep
            ));

            let max_intensity = peaks
                .iter()
                .map(|p| p[1].abs())
                .fold(0.0f64, f64::max)
                .max(1e-20);

            for (i, peak) in peaks.iter().enumerate() {
                out.push_str(&format!(
                    "{}{}  {:.4}{}  {:.6e}{}  {:.4}\n",
                    i + 1,
                    sep,
                    peak[0],
                    sep,
                    peak[1],
                    sep,
                    peak[1] / max_intensity * 100.0
                ));
            }
            out.push('\n');
        }

        // ── Integration Regions ──
        let integrations = &self.spectrum_view_state.integrations;
        if !integrations.is_empty() {
            out.push_str(&format!(
                "# Integration Regions ({} regions)\n",
                integrations.len()
            ));
            out.push_str(&format!(
                "Region_No{}Start_ppm{}End_ppm{}Absolute_Integral{}Relative_H{}Width_ppm\n",
                sep, sep, sep, sep, sep
            ));

            let first_raw = integrations
                .first()
                .map(|r| r.2)
                .unwrap_or(1.0)
                .abs()
                .max(1e-20);
            let ref_h = self.spectrum_view_state.integration_reference_h;

            for (i, &(start, end, raw_val)) in integrations.iter().enumerate() {
                let lo = start.min(end);
                let hi = start.max(end);
                out.push_str(&format!(
                    "{}{}  {:.4}{}  {:.4}{}  {:.6e}{}  {:.2}{}  {:.4}\n",
                    i + 1,
                    sep,
                    hi,  // higher ppm first (NMR convention)
                    sep,
                    lo,
                    sep,
                    raw_val,
                    sep,
                    (raw_val / first_raw) * ref_h,
                    sep,
                    hi - lo
                ));
            }
            out.push('\n');
        }

        // ── Multiplet Analysis ──
        let multiplets = &self.spectrum_view_state.multiplets;
        if !multiplets.is_empty() {
            out.push_str(&format!(
                "# Multiplet Analysis ({} multiplets)\n",
                multiplets.len()
            ));
            out.push_str(&format!(
                "Multiplet_No{}Center_ppm{}Multiplicity{}J_Hz{}Num_Lines{}Peak_PPMs\n",
                sep, sep, sep, sep, sep
            ));

            for (i, mult) in multiplets.iter().enumerate() {
                let peak_ppms: Vec<String> = mult
                    .peaks
                    .iter()
                    .map(|p| format!("{:.4}", p[0]))
                    .collect();
                out.push_str(&format!(
                    "{}{}  {:.4}{}  {}{}  {:.2}{}  {}{}  {}\n",
                    i + 1,
                    sep,
                    mult.center_ppm,
                    sep,
                    mult.label,
                    sep,
                    mult.j_hz,
                    sep,
                    mult.num_lines,
                    sep,
                    peak_ppms.join("; ")
                ));
            }
            out.push('\n');
        }

        // ── J-Coupling Measurements ──
        let j_couplings = &self.spectrum_view_state.j_couplings;
        if !j_couplings.is_empty() {
            out.push_str(&format!(
                "# J-Coupling Measurements ({} measurements)\n",
                j_couplings.len()
            ));
            out.push_str(&format!(
                "J_No{}Peak1_ppm{}Peak2_ppm{}Delta_ppm{}J_Hz\n",
                sep, sep, sep, sep
            ));

            for (i, &(ppm1, ppm2, delta, j_hz)) in j_couplings.iter().enumerate() {
                out.push_str(&format!(
                    "{}{}  {:.4}{}  {:.4}{}  {:.6}{}  {:.2}\n",
                    i + 1,
                    sep,
                    ppm1,
                    sep,
                    ppm2,
                    sep,
                    delta,
                    sep,
                    j_hz
                ));
            }
            out.push('\n');
        }

        // ── Summary ──
        if peaks.is_empty() && integrations.is_empty() && multiplets.is_empty() && j_couplings.is_empty() {
            out.push_str("# No peak, integration, multiplet, or J-coupling data to export.\n");
            out.push_str("# Run peak detection or define integrations first.\n");
        } else {
            out.push_str("# Summary\n");
            out.push_str(&format!("# Peaks: {}\n", peaks.len()));
            out.push_str(&format!("# Integrations: {}\n", integrations.len()));
            out.push_str(&format!("# Multiplets: {}\n", multiplets.len()));
            out.push_str(&format!("# J-Couplings: {}\n", j_couplings.len()));
        }

        std::fs::write(path, out).map_err(|e| e.to_string())
    }

    /// Handle pipeline actions
    fn handle_pipeline_action(&mut self, action: PipelineAction) {
        let spectrum = match self.spectrum.as_mut() {
            Some(s) => s,
            None => return,
        };

        match action {
            PipelineAction::ApplyApodization => {
                let wf = pipeline_panel::get_window_function(&self.pipeline_state);
                let op = ProcessingOp::Apodization(wf.clone());
                self.push_undo(op);
                let spectrum = self.spectrum.as_mut().unwrap();
                processing::apply_apodization(spectrum, &wf, &mut self.repro_log);
                self.status_message = format!("Applied apodization: {}", wf);
            }
            PipelineAction::ApplyZeroFill => {
                let current_size = spectrum.real.len();
                let target = current_size * (1 << self.pipeline_state.zf_factor);
                let op = ProcessingOp::ZeroFill {
                    target_size: target,
                };
                self.push_undo(op);
                let spectrum = self.spectrum.as_mut().unwrap();
                processing::zero_fill(spectrum, target, &mut self.repro_log);
                self.status_message = format!("Zero-filled to {} points", target);
            }
            PipelineAction::ApplyFT => {
                // Snapshot the FID before transforming so user can flip back
                if let Some(s) = &self.spectrum {
                    self.fid_snapshot = Some(s.clone());
                }
                let use_imaginary = self.pipeline_state.ft_use_imaginary;
                let op = ProcessingOp::FourierTransform { use_imaginary };
                self.push_undo(op);
                let spectrum = self.spectrum.as_mut().unwrap();
                processing::fourier_transform(spectrum, use_imaginary, &mut self.repro_log);
                self.status_message = format!(
                    "Fourier Transform applied ({})",
                    if use_imaginary { "Complex" } else { "Real-only" }
                );
                self.domain_tab = DomainTab::FrequencyDomain;
            }
            PipelineAction::ApplyFT2D => {
                // Snapshot the FID before transforming so user can undo
                if let Some(s) = &self.spectrum {
                    self.fid_snapshot = Some(s.clone());
                }
                let op = ProcessingOp::FourierTransform2D;
                self.push_undo(op);
                let spectrum = self.spectrum.as_mut().unwrap();
                let n_rows = spectrum.data_2d.len();
                let n_cols = spectrum.data_2d.first().map(|r| r.len()).unwrap_or(0);
                processing::fourier_transform_2d(spectrum, &mut self.repro_log);
                let new_rows = spectrum.data_2d.len();
                let new_cols = spectrum.data_2d.first().map(|r| r.len()).unwrap_or(0);
                self.status_message = format!(
                    "2D Fourier Transform: {}×{} → {}×{} (magnitude mode)",
                    n_rows, n_cols, new_rows, new_cols
                );
                self.domain_tab = DomainTab::FrequencyDomain;
            }
            PipelineAction::ApplyPhaseCorrection => {
                let ph0 = self.pipeline_state.ph0;
                let ph1 = self.pipeline_state.ph1;
                let op = ProcessingOp::PhaseCorrection { ph0, ph1 };
                self.push_undo(op);
                let spectrum = self.spectrum.as_mut().unwrap();
                processing::phase_correct(spectrum, ph0, ph1, &mut self.repro_log);
                self.status_message = format!("Phase correction: PH0={:.1}°, PH1={:.1}°", ph0, ph1);
            }
            PipelineAction::ApplyAutoPhase => {
                let op = ProcessingOp::AutoPhase;
                self.push_undo(op);
                let spectrum = self.spectrum.as_mut().unwrap();
                let (ph0, ph1) = processing::auto_phase(spectrum, &mut self.repro_log);
                self.pipeline_state.ph0 = ph0;
                self.pipeline_state.ph1 = ph1;
                self.status_message = format!("Auto phase: PH0={:.1}°, PH1={:.1}°", ph0, ph1);
            }
            PipelineAction::ApplyBaselineCorrection => {
                let op = ProcessingOp::BaselineCorrection;
                self.push_undo(op);
                let spectrum = self.spectrum.as_mut().unwrap();
                processing::baseline_correct(spectrum, &mut self.repro_log);
                self.status_message = "Baseline correction applied".to_string();
            }
            PipelineAction::ApplyManualBaseline => {
                let points = self.spectrum_view_state.baseline_points.clone();
                if points.len() < 2 {
                    self.status_message =
                        "Need at least 2 baseline points — click on the spectrum to add them"
                            .to_string();
                } else {
                    let op = ProcessingOp::ManualBaselineCorrection {
                        num_points: points.len(),
                    };
                    self.push_undo(op);
                    let spectrum = self.spectrum.as_mut().unwrap();
                    processing::manual_baseline_correct(spectrum, &points, &mut self.repro_log);
                    self.spectrum_view_state.baseline_points.clear();
                    self.spectrum_view_state.baseline_picking = false;
                    self.status_message = format!(
                        "Manual baseline correction applied ({} anchor points)",
                        points.len()
                    );
                }
            }
            PipelineAction::ToggleBaselinePicking => {
                self.spectrum_view_state.baseline_picking =
                    !self.spectrum_view_state.baseline_picking;
                if self.spectrum_view_state.baseline_picking {
                    // Disable other picking modes
                    self.spectrum_view_state.peak_picking = false;
                    self.spectrum_view_state.integration_picking = false;
                    self.spectrum_view_state.j_coupling_picking = false;
                    self.status_message =
                        "Baseline picking ON — click on the spectrum to place anchor points"
                            .to_string();
                } else {
                    self.status_message = "Baseline picking OFF".to_string();
                }
            }
            PipelineAction::ClearBaselinePoints => {
                self.spectrum_view_state.baseline_points.clear();
                self.status_message = "Baseline points cleared".to_string();
            }
            PipelineAction::ApplySolventSuppression => {
                let center = self.pipeline_state.solvent_center;
                let width = self.pipeline_state.solvent_width;
                let op = ProcessingOp::SolventSuppression {
                    center_ppm: center,
                    width_ppm: width,
                };
                self.push_undo(op);
                let spectrum = self.spectrum.as_mut().unwrap();
                processing::solvent_suppress(spectrum, center, width, &mut self.repro_log);
                self.status_message = format!("Solvent suppression at {:.2} ppm", center);
            }
            PipelineAction::DetectPeaks => {
                let threshold = self.pipeline_state.peak_threshold;
                let min_spacing_hz = self.pipeline_state.min_peak_spacing_hz;
                // Convert Hz to index distance using spectral width and data size
                let n = spectrum.real.len();
                let sw_hz = spectrum
                    .axes
                    .first()
                    .map(|a| a.spectral_width_hz)
                    .unwrap_or(n as f64);
                let pts_per_hz = if sw_hz > 0.0 { n as f64 / sw_hz } else { 1.0 };
                let min_dist = ((min_spacing_hz * pts_per_hz) as usize).max(2);
                let peaks = processing::detect_peaks(spectrum, threshold, min_dist);
                let peak_ppm_list: Vec<String> = peaks.iter().take(20).map(|p| format!("{:.3}", p[0])).collect();
                let desc = format!(
                    "Found {} peaks (threshold {:.0}%, min spacing {:.1} Hz): [{}]{}",
                    peaks.len(), threshold * 100.0, min_spacing_hz,
                    peak_ppm_list.join(", "),
                    if peaks.len() > 20 { "..." } else { "" }
                );
                self.repro_log.add_entry("Peak Detection", &desc, "# automatic peak picking (no NMRPipe equivalent)");
                self.status_message = format!(
                    "Detected {} peaks (threshold {:.0}%, min spacing {:.1} Hz)",
                    peaks.len(), threshold * 100.0, min_spacing_hz
                );
                self.spectrum_view_state.peaks = peaks;
            }
            PipelineAction::ClearPeaks => {
                let n = self.spectrum_view_state.peaks.len();
                self.spectrum_view_state.peaks.clear();
                self.spectrum_view_state.multiplets.clear();
                self.repro_log.add_entry("Clear Peaks", &format!("Cleared {} peaks and associated multiplets", n), "");
                self.status_message = "Peaks cleared".to_string();
            }
            PipelineAction::TogglePeakPicking => {
                self.spectrum_view_state.peak_picking =
                    !self.spectrum_view_state.peak_picking;
                if self.spectrum_view_state.peak_picking {
                    // Disable other picking modes
                    self.spectrum_view_state.baseline_picking = false;
                    self.spectrum_view_state.integration_picking = false;
                    self.spectrum_view_state.j_coupling_picking = false;
                    self.status_message =
                        "Peak picking ON — click to add peaks, Shift+click to remove nearest"
                            .to_string();
                } else {
                    self.status_message = "Peak picking OFF".to_string();
                }
            }
            PipelineAction::RemoveLastPeak => {
                if self.spectrum_view_state.peaks.pop().is_some() {
                    self.status_message = format!(
                        "Removed last peak ({} remaining)",
                        self.spectrum_view_state.peaks.len()
                    );
                } else {
                    self.status_message = "No peaks to remove".to_string();
                }
            }
            PipelineAction::DetectMultiplets => {
                // Detect peaks first if not done yet
                if self.spectrum_view_state.peaks.is_empty() {
                    let threshold = self.pipeline_state.peak_threshold;
                    let min_spacing_hz = self.pipeline_state.min_peak_spacing_hz;
                    let n = spectrum.real.len();
                    let sw_hz = spectrum
                        .axes
                        .first()
                        .map(|a| a.spectral_width_hz)
                        .unwrap_or(n as f64);
                    let pts_per_hz = if sw_hz > 0.0 { n as f64 / sw_hz } else { 1.0 };
                    let min_dist = ((min_spacing_hz * pts_per_hz) as usize).max(2);
                    self.spectrum_view_state.peaks =
                        processing::detect_peaks(spectrum, threshold, min_dist);
                }
                let obs_mhz = spectrum
                    .axes
                    .first()
                    .map(|a| a.observe_freq_mhz)
                    .unwrap_or(400.0);
                let multiplets = processing::detect_multiplets(
                    &self.spectrum_view_state.peaks,
                    20.0, // max J = 20 Hz
                    obs_mhz,
                );
                let summary: Vec<String> = multiplets.iter().map(|m| m.to_string()).collect();
                let desc = format!("Detected {} multiplets from {} peaks: {}",
                    multiplets.len(), self.spectrum_view_state.peaks.len(), summary.join("; "));
                self.repro_log.add_entry("Multiplet Detection", &desc, "# automatic multiplet analysis (no NMRPipe equivalent)");
                self.status_message = format!(
                    "Detected {} multiplets: {}",
                    multiplets.len(),
                    summary.join("; ")
                );
                self.spectrum_view_state.multiplets = multiplets;
            }
            PipelineAction::ClearMultiplets => {
                let n = self.spectrum_view_state.multiplets.len();
                self.spectrum_view_state.multiplets.clear();
                self.repro_log.add_entry("Clear Multiplets", &format!("Cleared {} multiplets", n), "");
                self.status_message = "Multiplets cleared".to_string();
            }
            PipelineAction::ToggleIntegrationPicking => {
                self.spectrum_view_state.integration_picking =
                    !self.spectrum_view_state.integration_picking;
                if self.spectrum_view_state.integration_picking {
                    // Disable other picking modes
                    self.spectrum_view_state.peak_picking = false;
                    self.spectrum_view_state.baseline_picking = false;
                    self.spectrum_view_state.j_coupling_picking = false;
                    self.spectrum_view_state.integration_start = None;
                    self.status_message =
                        "Integration picking ON — click start and end points on the spectrum"
                            .to_string();
                } else {
                    self.spectrum_view_state.integration_start = None;
                    self.status_message = "Integration picking OFF".to_string();
                }
            }
            PipelineAction::ClearIntegrations => {
                let n = self.spectrum_view_state.integrations.len();
                self.spectrum_view_state.integrations.clear();
                self.spectrum_view_state.integration_start = None;
                self.repro_log.add_entry("Clear Integrations", &format!("Cleared {} integration regions", n), "");
                self.status_message = "Integrations cleared".to_string();
            }
            PipelineAction::ToggleJCouplingPicking => {
                self.spectrum_view_state.j_coupling_picking =
                    !self.spectrum_view_state.j_coupling_picking;
                if self.spectrum_view_state.j_coupling_picking {
                    // Disable other picking modes
                    self.spectrum_view_state.peak_picking = false;
                    self.spectrum_view_state.baseline_picking = false;
                    self.spectrum_view_state.integration_picking = false;
                    self.spectrum_view_state.j_coupling_first = None;
                    self.status_message =
                        "J-coupling measurement ON — click two peaks to measure spacing"
                            .to_string();
                } else {
                    self.spectrum_view_state.j_coupling_first = None;
                    self.status_message = "J-coupling measurement OFF".to_string();
                }
            }
            PipelineAction::ClearJCouplings => {
                let n = self.spectrum_view_state.j_couplings.len();
                self.spectrum_view_state.j_couplings.clear();
                self.spectrum_view_state.j_coupling_first = None;
                self.repro_log.add_entry("Clear J-Couplings", &format!("Cleared {} J-coupling measurements", n), "");
                self.status_message = "J-coupling measurements cleared".to_string();
            }
            PipelineAction::None => {}
        }
    }

    /// Save the current project (spectrum + annotations) to a JSON file
    fn save_project(&self, path: &std::path::Path) -> Result<(), String> {
        let save = ProjectSave {
            spectrum: self.spectrum.clone(),
            fid_snapshot: self.fid_snapshot.clone(),
            is_frequency_domain: self.spectrum.as_ref().map(|s| s.is_frequency_domain).unwrap_or(false),
            peaks: self.spectrum_view_state.peaks.clone(),
            multiplets: self.spectrum_view_state.multiplets.clone(),
            integrations: self.spectrum_view_state.integrations.clone(),
            integration_reference_h: self.spectrum_view_state.integration_reference_h,
            j_couplings: self.spectrum_view_state.j_couplings.clone(),
            baseline_points: self.spectrum_view_state.baseline_points.clone(),
            theme: format!("{:?}", self.current_theme),
            sample_name: self.spectrum.as_ref().map(|s| s.sample_name.clone()).unwrap_or_default(),
        };
        let json = serde_json::to_string_pretty(&save).map_err(|e| format!("Serialize error: {}", e))?;
        std::fs::write(path, json).map_err(|e| format!("Write error: {}", e))?;
        Ok(())
    }

    /// Load a project from a JSON file
    fn load_project(&mut self, path: &std::path::Path) -> Result<(), String> {
        let json = std::fs::read_to_string(path).map_err(|e| format!("Read error: {}", e))?;
        let save: ProjectSave = serde_json::from_str(&json).map_err(|e| format!("Parse error: {}", e))?;

        self.spectrum = save.spectrum;
        self.fid_snapshot = save.fid_snapshot;
        self.spectrum_view_state.peaks = save.peaks;
        self.spectrum_view_state.multiplets = save.multiplets;
        self.spectrum_view_state.integrations = save.integrations;
        self.spectrum_view_state.integration_reference_h = save.integration_reference_h;
        self.spectrum_view_state.j_couplings = save.j_couplings;
        self.spectrum_view_state.baseline_points = save.baseline_points;
        self.spectrum_view_state.auto_scale = true;

        // Reset picking modes from previous session
        self.spectrum_view_state.peak_picking = false;
        self.spectrum_view_state.baseline_picking = false;
        self.spectrum_view_state.integration_picking = false;
        self.spectrum_view_state.j_coupling_picking = false;
        self.spectrum_view_state.integration_start = None;
        self.spectrum_view_state.j_coupling_first = None;

        // Reset phase dialog
        self.phase_dialog_state = PhaseDialogState::default();

        // Reset pipeline state
        self.pipeline_state = PipelinePanelState::default();

        // Restore domain tab
        if save.is_frequency_domain {
            self.domain_tab = DomainTab::FrequencyDomain;
        } else {
            self.domain_tab = DomainTab::TimeDomain;
        }

        // Restore theme
        let new_theme = if save.theme.contains("Cyberpunk") {
            AppTheme::Cyberpunk
        } else {
            AppTheme::Light
        };
        self.current_theme = new_theme;
        self.theme_colors = ThemeColors::from_theme(new_theme);

        // Reset processing state
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.before_snapshot = None;
        self.repro_log = ReproLog::new();

        Ok(())
    }

    /// Handle toolbar actions
    fn handle_toolbar_action(&mut self, action: ToolbarAction) {
        match action {
            ToolbarAction::OpenFile => {
                if let Some(path) = toolbar::open_file_dialog() {
                    self.load_path(path);
                }
            }
            ToolbarAction::OpenFolder => {
                if let Some(path) = toolbar::open_folder_dialog() {
                    self.load_path(path);
                }
            }
            ToolbarAction::SaveProject => {
                if self.spectrum.is_some() {
                    let default_name = self.spectrum.as_ref()
                        .map(|s| format!("{}.nmrproj", s.sample_name))
                        .unwrap_or_else(|| "project.nmrproj".to_string());
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Save Project")
                        .set_file_name(&default_name)
                        .add_filter("NMR Project", &["nmrproj"])
                        .save_file()
                    {
                        match self.save_project(&path) {
                            Ok(_) => self.status_message = format!("Project saved: {}", path.display()),
                            Err(e) => self.status_message = format!("Save failed: {}", e),
                        }
                    }
                } else {
                    self.status_message = "No spectrum loaded to save".to_string();
                }
            }
            ToolbarAction::LoadProject => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Load Project")
                    .add_filter("NMR Project", &["nmrproj"])
                    .pick_file()
                {
                    match self.load_project(&path) {
                        Ok(_) => {
                            let name = self.spectrum.as_ref()
                                .map(|s| s.sample_name.clone())
                                .unwrap_or_else(|| "unknown".to_string());
                            self.status_message = format!("Project loaded: {}", name);
                        }
                        Err(e) => self.status_message = format!("Load failed: {}", e),
                    }
                }
            }
            ToolbarAction::ExportImage => {
                if self.spectrum.is_some() {
                    // Initialize ppm range from spectrum data
                    if let Some(spectrum) = &self.spectrum {
                        if spectrum.is_frequency_domain && !spectrum.axes.is_empty() {
                            let ppm_scale = spectrum.axes[0].ppm_scale();
                            let ppm_min = ppm_scale.iter().cloned().fold(f64::INFINITY, f64::min);
                            let ppm_max = ppm_scale.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                            self.export_tab_state.image_settings.ppm_start = ppm_max;
                            self.export_tab_state.image_settings.ppm_end = ppm_min;
                        }
                    }
                    self.export_tab_state.active_section = 0;
                    self.domain_tab = DomainTab::Export;
                } else {
                    self.status_message = "No spectrum loaded to export".to_string();
                }
            }
            ToolbarAction::ExportData => {
                if self.spectrum.is_some() {
                    self.export_tab_state.active_section = 1;
                    self.domain_tab = DomainTab::Export;
                } else {
                    self.status_message = "No spectrum loaded to export".to_string();
                }
            }
            ToolbarAction::ExportLog => {
                if let Some(path) = toolbar::save_log_dialog() {
                    let ext = path
                        .extension()
                        .map(|e| e.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    let result = match ext.as_str() {
                        "json" => self.repro_log.save_json(&path),
                        "sh" => self.repro_log.save_script(&path),
                        _ => self.repro_log.save_text(&path),
                    };
                    match result {
                        Ok(_) => {
                            self.status_message = format!("Log saved: {}", path.display());
                        }
                        Err(e) => {
                            self.status_message = format!("Error saving log: {}", e);
                        }
                    }
                }
            }
            ToolbarAction::Undo => self.undo(),
            ToolbarAction::Redo => self.redo(),
            ToolbarAction::ShowAbout => {
                self.show_about = true;
            }
            ToolbarAction::ThemeToggle => {
                self.current_theme = self.current_theme.next();
                self.theme_colors = ThemeColors::from_theme(self.current_theme);
                // apply_theme needs a reference to ctx, but we don't have it here;
                // we'll apply it lazily on next frame via update()
            }
            ToolbarAction::ToggleConversionMethod => {
                use crate::gui::conversion_dialog::ConversionMethod;
                self.conversion_method = match self.conversion_method {
                    ConversionMethod::NMRPipe => ConversionMethod::BuiltIn,
                    ConversionMethod::BuiltIn => ConversionMethod::NMRPipe,
                };
                self.status_message = format!(
                    "Conversion method: {} — reload file to apply",
                    self.conversion_method.label()
                );
            }
            ToolbarAction::ZoomReset => {
                self.spectrum_view_state.auto_scale = true;
                self.status_message = "Zoom reset".to_string();
            }
            ToolbarAction::None => {}
        }
    }

    /// Handle interactive phase correction
    fn handle_phase_action(&mut self, action: PhaseAction) {
        match action {
            PhaseAction::Start => {
                if let Some(spectrum) = &self.spectrum {
                    self.phase_dialog_state.compute_preview(spectrum);
                }
            }
            PhaseAction::UpdatePreview => {
                if let Some(spectrum) = &self.spectrum {
                    self.phase_dialog_state.compute_preview(spectrum);
                }
            }
            PhaseAction::Apply => {
                let ph0 = self.phase_dialog_state.ph0;
                let ph1 = self.phase_dialog_state.ph1;
                self.phase_dialog_state.active = false;

                // Apply the phase correction permanently
                let op = ProcessingOp::PhaseCorrection { ph0, ph1 };
                self.push_undo(op);
                if let Some(spectrum) = self.spectrum.as_mut() {
                    processing::phase_correct(spectrum, ph0, ph1, &mut self.repro_log);
                }
                self.pipeline_state.ph0 = ph0;
                self.pipeline_state.ph1 = ph1;
                self.status_message =
                    format!("Interactive phase applied: PH0={:.1}°, PH1={:.1}°", ph0, ph1);
            }
            PhaseAction::Cancel => {
                self.phase_dialog_state.active = false;
                self.phase_dialog_state.ph0 = 0.0;
                self.phase_dialog_state.ph1 = 0.0;
                self.phase_dialog_state.preview.clear();
            }
            PhaseAction::None => {}
        }
    }
}

impl eframe::App for NmrApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Re-apply theme each frame (ensures toggle takes effect) ──
        theme::apply_theme(ctx, self.current_theme);

        // Handle drag-and-drop
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                for file in &i.raw.dropped_files {
                    if let Some(path) = &file.path {
                        self.dropped_files.push(path.clone());
                    }
                }
            }
        });

        // Process dropped files
        if let Some(path) = self.dropped_files.pop() {
            self.load_path(path);
        }

        // ── Conversion Dialog ──
        let conv_action =
            conversion_dialog::show_conversion_dialog(ctx, &mut self.conversion_dialog_state);
        match conv_action {
            ConversionAction::Convert => {
                let path = self.conversion_dialog_state.pending_path.take();
                let settings = self.conversion_dialog_state.settings.clone();
                self.conversion_dialog_state.open = false;
                if let Some(path) = path {
                    self.do_load(&path, Some(&settings));
                }
            }
            ConversionAction::Cancel => {
                self.conversion_dialog_state.open = false;
                self.conversion_dialog_state.pending_path = None;
                self.status_message = "Conversion cancelled".to_string();
            }
            ConversionAction::None => {}
        }

        // ── Export Dialog ──
        let has_peaks = !self.spectrum_view_state.peaks.is_empty();
        let has_integrations = !self.spectrum_view_state.integrations.is_empty();
        let has_multiplets = !self.spectrum_view_state.multiplets.is_empty();
        let export_action = export_dialog::show_export_dialog(
            ctx,
            &mut self.export_dialog_state,
            has_peaks,
            has_integrations,
            has_multiplets,
        );
        match export_action {
            ExportAction::Export => {
                self.export_dialog_state.open = false;
                // Ask for save path based on format
                let dialog = if self.export_dialog_state.settings.format == 1 {
                    rfd::FileDialog::new()
                        .set_title("Export Spectrum Image")
                        .add_filter("SVG Image", &["svg"])
                        .save_file()
                } else {
                    rfd::FileDialog::new()
                        .set_title("Export Spectrum Image")
                        .add_filter("PNG Image", &["png"])
                        .save_file()
                };
                if let Some(path) = dialog {
                    let settings = self.export_dialog_state.settings.clone();
                    match self.export_spectrum_image_with_settings(&path, &settings) {
                        Ok(_) => {
                            self.status_message = format!("Image exported: {}", path.display());
                            self.repro_log.add_entry(
                                "Export Image",
                                &format!("Exported spectrum image to {}", path.display()),
                                "",
                            );
                        }
                        Err(e) => {
                            self.status_message = format!("Image export failed: {}", e);
                        }
                    }
                }
            }
            ExportAction::Cancel => {
                self.export_dialog_state.open = false;
            }
            ExportAction::None => {}
        }

        // ── Toolbar ──
        let theme_label = self.current_theme.label();
        let method_label = self.conversion_method.short_label();
        let toolbar_action = toolbar::show_toolbar(
            ctx,
            theme_label,
            method_label,
            !self.undo_stack.is_empty(),
            !self.redo_stack.is_empty(),
        );
        if toolbar_action != ToolbarAction::None {
            self.handle_toolbar_action(toolbar_action);
        }

        // ── Status Bar ──
        let tc = &self.theme_colors;
        let phase_active = self.phase_dialog_state.active;
        let cursor_mode = theme::cursor_mode_label(&self.spectrum_view_state, phase_active);
        let sb_bg = tc.status_bar_bg;
        let sb_text = tc.status_text;
        let sb_muted = tc.text_muted;
        let sb_success = tc.success;
        let sb_warning = tc.warning;

        egui::TopBottomPanel::bottom("status_bar")
            .frame(egui::Frame::new()
                .fill(sb_bg)
                .inner_margin(egui::Margin::symmetric(12, 4)))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Cursor mode badge (prominent, colored)
                if let Some((mode_name, mode_hint, mode_color)) = cursor_mode {
                    let badge_bg = mode_color.linear_multiply(0.2);
                    let badge = egui::Button::new(
                        egui::RichText::new(mode_name)
                            .size(11.5)
                            .strong()
                            .color(mode_color),
                    )
                    .fill(badge_bg)
                    .stroke(egui::Stroke::new(1.0, mode_color))
                    .corner_radius(10.0);
                    ui.add(badge);
                    ui.label(
                        egui::RichText::new(mode_hint)
                            .size(11.0)
                            .italics()
                            .color(mode_color.linear_multiply(0.7)),
                    );
                    ui.separator();
                }

                ui.label(
                    egui::RichText::new(&self.status_message)
                        .size(11.5)
                        .color(sb_text),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Conversion method indicator (clickable to toggle)
                    {
                        use crate::gui::conversion_dialog::ConversionMethod;
                        let (method_icon, method_text, method_color) = match self.conversion_method {
                            ConversionMethod::NMRPipe => ("⚡", "NMRPipe", sb_success),
                            ConversionMethod::BuiltIn => ("📦", "Built-in", sb_warning),
                        };
                        let badge = egui::Button::new(
                            egui::RichText::new(format!("{} {}", method_icon, method_text))
                                .size(11.0)
                                .color(method_color),
                        )
                        .fill(method_color.linear_multiply(0.1))
                        .stroke(egui::Stroke::new(1.0, method_color.linear_multiply(0.3)))
                        .corner_radius(8.0);
                        if ui.add(badge).on_hover_text(
                            "Click to switch conversion method.\n\
                             NMRPipe: uses bruk2pipe / delta2pipe / var2pipe\n\
                             Built-in: native readers, no NMRPipe required"
                        ).clicked() {
                            self.conversion_method = match self.conversion_method {
                                ConversionMethod::NMRPipe => ConversionMethod::BuiltIn,
                                ConversionMethod::BuiltIn => ConversionMethod::NMRPipe,
                            };
                            self.status_message = format!(
                                "Conversion method: {} — reload file to apply",
                                self.conversion_method.label()
                            );
                        }
                    }
                    // Show what method was used for current spectrum
                    if let Some(spectrum) = &self.spectrum {
                        if !spectrum.conversion_method_used.is_empty() {
                            ui.separator();
                            ui.label(
                                egui::RichText::new(format!("via {}", spectrum.conversion_method_used))
                                    .size(10.0)
                                    .italics()
                                    .color(sb_muted),
                            );
                        }
                    }
                    ui.separator();
                    if self.nmrpipe_available {
                        ui.colored_label(
                            sb_success,
                            egui::RichText::new("● NMRPipe").size(11.0),
                        );
                    } else {
                        ui.colored_label(
                            sb_warning,
                            egui::RichText::new("○ no NMRPipe").size(11.0),
                        );
                    }
                    ui.separator();
                    if ui.small_button("📋 Log").clicked() {
                        self.show_log_window = !self.show_log_window;
                    }
                    ui.label(
                        egui::RichText::new(format!("{} ops", self.repro_log.len()))
                            .size(11.0)
                            .color(sb_muted),
                    );
                });
            });
        });

        // ── Left Panel: Processing Pipeline ──
        let has_data = self.spectrum.is_some();
        let is_freq = self
            .spectrum
            .as_ref()
            .map(|s| s.is_frequency_domain)
            .unwrap_or(false);
        let op_count = self.repro_log.len();

        let is_2d = self
            .spectrum
            .as_ref()
            .map(|s| s.is_2d())
            .unwrap_or(false);

        let mut pipeline_action_deferred = PipelineAction::None;
        let picking_modes = pipeline_panel::PickingModes {
            peak_picking: self.spectrum_view_state.peak_picking,
            baseline_picking: self.spectrum_view_state.baseline_picking,
            integration_picking: self.spectrum_view_state.integration_picking,
            j_coupling_picking: self.spectrum_view_state.j_coupling_picking,
        };
        egui::SidePanel::left("pipeline_panel")
            .resizable(true)
            .default_width(260.0)
            .min_width(200.0)
            .max_width(400.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                    pipeline_action_deferred = pipeline_panel::show_pipeline_panel(
                        ui,
                        &mut self.pipeline_state,
                        has_data,
                        is_freq,
                        is_2d,
                        op_count,
                        &picking_modes,
                        &mut self.spectrum_view_state.integration_reference_h,
                        self.before_snapshot.is_some(),
                    );
                });
            });

        // ── Central Panel: Spectrum Display with Domain Tabs ──
        let mut phase_action_deferred = PhaseAction::None;
        let tab_active_bg = self.theme_colors.tab_active_bg;
        let tab_active_text = self.theme_colors.tab_active_text;
        let tab_inactive_bg = self.theme_colors.tab_inactive_bg;
        let tab_inactive_text = self.theme_colors.tab_inactive_text;
        egui::CentralPanel::default().show(ctx, |ui| {
            // Domain tabs: show only when we have a FID snapshot (i.e. after FT)
            if self.fid_snapshot.is_some() && self.spectrum.is_some() {
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    // Time Domain tab
                    let td_active = self.domain_tab == DomainTab::TimeDomain;
                    let td_label = egui::RichText::new("📈 FID (Time Domain)")
                        .size(13.0)
                        .color(if td_active { tab_active_text } else { tab_inactive_text });
                    let td_btn = egui::Button::new(td_label)
                        .fill(if td_active { tab_active_bg } else { tab_inactive_bg })
                        .corner_radius(6.0);
                    if ui.add(td_btn).clicked() {
                        self.domain_tab = DomainTab::TimeDomain;
                        self.spectrum_view_state.auto_scale = true;
                        // Reset picking modes when switching tabs
                        self.spectrum_view_state.peak_picking = false;
                        self.spectrum_view_state.baseline_picking = false;
                        self.spectrum_view_state.integration_picking = false;
                        self.spectrum_view_state.j_coupling_picking = false;
                        self.spectrum_view_state.integration_start = None;
                        self.spectrum_view_state.j_coupling_first = None;
                    }

                    ui.add_space(4.0);

                    // Frequency Domain tab
                    let fd_active = self.domain_tab == DomainTab::FrequencyDomain;
                    let fd_label = egui::RichText::new("📊 Spectrum (Freq Domain)")
                        .size(13.0)
                        .color(if fd_active { tab_active_text } else { tab_inactive_text });
                    let fd_btn = egui::Button::new(fd_label)
                        .fill(if fd_active { tab_active_bg } else { tab_inactive_bg })
                        .corner_radius(6.0);
                    if ui.add(fd_btn).clicked() {
                        self.domain_tab = DomainTab::FrequencyDomain;
                        self.spectrum_view_state.auto_scale = true;
                        // Reset picking modes when switching tabs
                        self.spectrum_view_state.peak_picking = false;
                        self.spectrum_view_state.baseline_picking = false;
                        self.spectrum_view_state.integration_picking = false;
                        self.spectrum_view_state.j_coupling_picking = false;
                        self.spectrum_view_state.integration_start = None;
                        self.spectrum_view_state.j_coupling_first = None;
                    }

                    ui.add_space(4.0);

                    // Export tab
                    let ex_active = self.domain_tab == DomainTab::Export;
                    let ex_label = egui::RichText::new("📥 Export")
                        .size(13.0)
                        .color(if ex_active { tab_active_text } else { tab_inactive_text });
                    let ex_btn = egui::Button::new(ex_label)
                        .fill(if ex_active { tab_active_bg } else { tab_inactive_bg })
                        .corner_radius(6.0);
                    if ui.add(ex_btn).clicked() {
                        // Initialize PPM range on first switch
                        if let Some(spectrum) = &self.spectrum {
                            if spectrum.is_frequency_domain && !spectrum.axes.is_empty() {
                                let ppm_scale = spectrum.axes[0].ppm_scale();
                                let ppm_min = ppm_scale.iter().cloned().fold(f64::INFINITY, f64::min);
                                let ppm_max = ppm_scale.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                                if !self.export_tab_state.image_settings.use_custom_range {
                                    self.export_tab_state.image_settings.ppm_start = ppm_max;
                                    self.export_tab_state.image_settings.ppm_end = ppm_min;
                                }
                            }
                        }
                        self.domain_tab = DomainTab::Export;
                    }
                });
                ui.add_space(2.0);

                // Also show Export tab even without FID snapshot (freq-only data)
            } else if self.spectrum.is_some() {
                // No FID snapshot (haven't done FT) — show just export button
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    let ex_active = self.domain_tab == DomainTab::Export;
                    let ex_label = egui::RichText::new("📥 Export")
                        .size(13.0)
                        .color(if ex_active { tab_active_text } else { tab_inactive_text });
                    let ex_btn = egui::Button::new(ex_label)
                        .fill(if ex_active { tab_active_bg } else { tab_inactive_bg })
                        .corner_radius(6.0);
                    if ui.add(ex_btn).clicked() {
                        if let Some(spectrum) = &self.spectrum {
                            if spectrum.is_frequency_domain && !spectrum.axes.is_empty() {
                                let ppm_scale = spectrum.axes[0].ppm_scale();
                                let ppm_min = ppm_scale.iter().cloned().fold(f64::INFINITY, f64::min);
                                let ppm_max = ppm_scale.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                                if !self.export_tab_state.image_settings.use_custom_range {
                                    self.export_tab_state.image_settings.ppm_start = ppm_max;
                                    self.export_tab_state.image_settings.ppm_end = ppm_min;
                                }
                            }
                        }
                        self.domain_tab = DomainTab::Export;
                    }
                });
                ui.add_space(2.0);
            }

            // Determine which spectrum to display
            let display_spectrum = if self.domain_tab == DomainTab::TimeDomain
                && self.fid_snapshot.is_some()
            {
                self.fid_snapshot.as_ref()
            } else {
                self.spectrum.as_ref()
            };

            if self.domain_tab == DomainTab::Export {
                // ── Export Tab ──
                if let Some(spectrum) = self.spectrum.as_ref() {
                    let export_action = export_tab::show_export_tab(
                        ui,
                        &mut self.export_tab_state,
                        spectrum,
                        &self.spectrum_view_state,
                    );
                    match export_action {
                        ExportTabAction::ExportImage => {
                            let s = &self.export_tab_state.image_settings;
                            let dialog = if s.format == 1 {
                                rfd::FileDialog::new()
                                    .set_title("Export Spectrum Image")
                                    .add_filter("SVG Image", &["svg"])
                                    .save_file()
                            } else {
                                rfd::FileDialog::new()
                                    .set_title("Export Spectrum Image")
                                    .add_filter("PNG Image", &["png"])
                                    .save_file()
                            };
                            if let Some(path) = dialog {
                                // Convert to old ExportSettings for existing export methods
                                let settings = ExportSettings {
                                    ppm_start: s.ppm_start,
                                    ppm_end: s.ppm_end,
                                    use_custom_range: s.use_custom_range,
                                    width: s.width,
                                    height: s.height,
                                    show_peaks: s.show_peaks,
                                    show_integrations: s.show_integrations,
                                    show_multiplets: s.show_multiplets,
                                    custom_title: s.custom_title.clone(),
                                    use_custom_title: s.use_custom_title,
                                    line_width: s.line_width,
                                    show_grid: s.show_grid,
                                    format: s.format,
                                    clip_negatives: s.clip_negatives,
                                    dpi: s.dpi,
                                    marker_scale: s.marker_scale,
                                    font_scale: s.font_scale,
                                };
                                match self.export_spectrum_image_with_settings(&path, &settings) {
                                    Ok(_) => {
                                        self.status_message = format!("✅ Image exported: {}", path.display());
                                        self.repro_log.add_entry(
                                            "Export Image",
                                            &format!("Exported spectrum image to {}", path.display()),
                                            "",
                                        );
                                    }
                                    Err(e) => {
                                        self.status_message = format!("❌ Image export failed: {}", e);
                                    }
                                }
                            }
                        }
                        ExportTabAction::ExportData => {
                            if let Some(path) = toolbar::save_data_dialog() {
                                match self.export_data_report(&path) {
                                    Ok(_) => {
                                        self.status_message = format!("✅ Data exported: {}", path.display());
                                        self.repro_log.add_entry(
                                            "Export Data",
                                            &format!("Exported peak/integration data to {}", path.display()),
                                            "",
                                        );
                                    }
                                    Err(e) => {
                                        self.status_message = format!("❌ Data export failed: {}", e);
                                    }
                                }
                            }
                        }
                        ExportTabAction::ExportLog => {
                            if let Some(path) = toolbar::save_log_dialog() {
                                let ext = path
                                    .extension()
                                    .map(|e| e.to_string_lossy().to_lowercase())
                                    .unwrap_or_default();
                                let result = match ext.as_str() {
                                    "json" => self.repro_log.save_json(&path),
                                    "sh" => self.repro_log.save_script(&path),
                                    _ => self.repro_log.save_text(&path),
                                };
                                match result {
                                    Ok(_) => {
                                        self.status_message = format!("✅ Log saved: {}", path.display());
                                    }
                                    Err(e) => {
                                        self.status_message = format!("❌ Error saving log: {}", e);
                                    }
                                }
                            }
                        }
                        ExportTabAction::None => {}
                    }
                }
            } else if let Some(spectrum) = display_spectrum {
                // Interactive phase controls (available on any 1D data — time or freq domain)
                if !spectrum.is_2d() {
                    let phase_action =
                        phase_dialog::show_phase_controls(ui, &mut self.phase_dialog_state);
                    if phase_action != PhaseAction::None {
                        phase_action_deferred = phase_action;
                    }
                }

                if spectrum.is_2d() {
                    // 2D contour display
                    let ft_requested = contour_view::show_spectrum_2d(ui, spectrum, &mut self.contour_view_state);
                    if ft_requested {
                        pipeline_action_deferred = PipelineAction::ApplyFT2D;
                    }
                } else {
                    // 1D spectrum display
                    let before = if self.pipeline_state.show_before_after {
                        self.before_snapshot.as_ref()
                    } else {
                        None
                    };
                    spectrum_view::show_spectrum_1d(
                        ui,
                        spectrum,
                        before,
                        &mut self.spectrum_view_state,
                        self.pipeline_state.show_before_after,
                        &mut self.phase_dialog_state,
                        &self.theme_colors,
                    );

                    // Drain pending analysis actions from click handlers and log them
                    for action in self.spectrum_view_state.pending_actions.drain(..) {
                        match action {
                            spectrum_view::SpectrumAction::PeakAdded(peak) => {
                                self.repro_log.add_entry(
                                    "Manual Peak Pick",
                                    &format!("Added peak at {:.4} ppm (intensity {:.1})", peak[0], peak[1]),
                                    "# manual peak pick (no NMRPipe equivalent)",
                                );
                            }
                            spectrum_view::SpectrumAction::PeakRemoved(ppm) => {
                                self.repro_log.add_entry(
                                    "Manual Peak Remove",
                                    &format!("Removed peak near {:.4} ppm", ppm),
                                    "# manual peak removal (no NMRPipe equivalent)",
                                );
                            }
                            spectrum_view::SpectrumAction::IntegrationAdded(lo, hi, raw) => {
                                self.repro_log.add_entry(
                                    "Integration",
                                    &format!("Integrated region {:.4}–{:.4} ppm (raw area = {:.2})", lo, hi, raw),
                                    "# manual integration (no NMRPipe equivalent)",
                                );
                            }
                            spectrum_view::SpectrumAction::JCouplingMeasured(ppm1, ppm2, _dppm, j_hz) => {
                                self.repro_log.add_entry(
                                    "J-Coupling Measurement",
                                    &format!("Measured J = {:.1} Hz between {:.4} and {:.4} ppm", j_hz, ppm1, ppm2),
                                    "# J-coupling measurement (no NMRPipe equivalent)",
                                );
                            }
                        }
                    }
                }
            } else {
                // Welcome screen
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.28);
                    ui.heading(
                        egui::RichText::new("🧪 NMR Spectral Processing")
                            .size(26.0)
                            .color(self.theme_colors.text_heading),
                    );
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new("Drag & drop an NMR data file or folder here")
                            .size(14.5)
                            .color(self.theme_colors.text_muted),
                    );
                    ui.label(
                        egui::RichText::new("or use File → Open")
                            .size(14.5)
                            .color(self.theme_colors.text_muted),
                    );
                    ui.add_space(24.0);
                    ui.label(
                        egui::RichText::new("JEOL (.jdf)  ·  Bruker  ·  Varian/Agilent  ·  NMRPipe")
                            .size(12.0)
                            .color(self.theme_colors.accent_dim),
                    );
                    ui.add_space(16.0);
                    if self.nmrpipe_available {
                        ui.colored_label(
                            self.theme_colors.success,
                            egui::RichText::new("✅ NMRPipe detected").size(12.0),
                        );
                    } else {
                        ui.colored_label(
                            self.theme_colors.warning,
                            egui::RichText::new("⚠ NMRPipe not found — using built-in processing").size(12.0),
                        );
                    }
                });
            }
        });

        // Handle deferred actions
        if pipeline_action_deferred != PipelineAction::None {
            // Switch to frequency domain view when applying freq-domain ops
            if self.spectrum.as_ref().map(|s| s.is_frequency_domain).unwrap_or(false) {
                self.domain_tab = DomainTab::FrequencyDomain;
            }
            self.handle_pipeline_action(pipeline_action_deferred);
        }
        if phase_action_deferred != PhaseAction::None {
            self.handle_phase_action(phase_action_deferred);
        }

        // ── Log Window ──
        if self.show_log_window {
            egui::Window::new("📋 Reproducibility Log")
                .open(&mut self.show_log_window)
                .default_size([600.0, 400.0])
                .resizable(true)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("💾 Save as Text").clicked() {
                            if let Some(path) = toolbar::save_log_dialog() {
                                let _ = self.repro_log.save_text(&path);
                            }
                        }
                        if ui.button("💾 Save as JSON").clicked() {
                            if let Some(path) = toolbar::save_log_dialog() {
                                let _ = self.repro_log.save_json(&path);
                            }
                        }
                        if ui.button("💾 Save as Script").clicked() {
                            if let Some(path) = toolbar::save_log_dialog() {
                                let _ = self.repro_log.save_script(&path);
                            }
                        }
                    });
                    ui.separator();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.style_mut().override_font_id =
                            Some(egui::FontId::monospace(12.0));
                        ui.label(self.repro_log.to_text());
                    });
                });
        }

        // ── About Dialog ──
        if self.show_about {
            egui::Window::new("About")
                .open(&mut self.show_about)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.heading("🧪 NMR Spectral Processing GUI");
                    ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                    ui.add_space(10.0);
                    ui.label("Built with Rust + egui");
                    ui.label("NMRPipe backend for processing");
                    ui.add_space(10.0);
                    ui.label("Features:");
                    ui.label("• Automatic format detection & conversion");
                    ui.label("• Full processing pipeline (FT, phase, baseline, etc.)");
                    ui.label("• Mandatory reproducibility logging");
                    ui.label("• Interactive spectrum display with zoom/pan");
                    ui.label("• Undo/redo with full state snapshots");
                    ui.label("• Time / Frequency domain tabs");
                });
        }

        // Handle keyboard shortcuts
        ctx.input(|i| {
            if i.modifiers.ctrl || i.modifiers.command {
                if i.key_pressed(egui::Key::Z) {
                    if i.modifiers.shift {
                        self.redo();
                    } else {
                        self.undo();
                    }
                }
                if i.key_pressed(egui::Key::O) {
                    if let Some(path) = toolbar::open_file_dialog() {
                        self.load_path(path);
                    }
                }
            }
        });
    }
}

/// Choose a nice tick spacing for x-axis labels given the total ppm range.
fn smart_tick_step(range: f64) -> f64 {
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

/// Draw a line between two points on an RgbImage using Bresenham's algorithm.
fn draw_line(
    img: &mut image::RgbImage,
    x0: i32, y0: i32,
    x1: i32, y1: i32,
    color: image::Rgb<u8>,
    w: u32, h: u32,
) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut cx = x0;
    let mut cy = y0;
    loop {
        if cx >= 0 && cx < w as i32 && cy >= 0 && cy < h as i32 {
            img.put_pixel(cx as u32, cy as u32, color);
        }
        if cx == x1 && cy == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            cx += sx;
        }
        if e2 <= dx {
            err += dx;
            cy += sy;
        }
    }
}

/// Very simple built-in 3×5 bitmap font for labeling exported images.
fn draw_simple_text(img: &mut image::RgbImage, text: &str, x: u32, y: u32, color: image::Rgb<u8>, text_scale: u32) {
    // Minimal 3×5 font for digits, letters, and common symbols
    let glyph = |c: char| -> [u8; 5] {
        match c {
            '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
            '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
            '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
            '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
            '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
            '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
            '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
            '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
            '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
            '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
            '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
            ',' => [0b000, 0b000, 0b000, 0b010, 0b100],
            '-' | '—' | '–' => [0b000, 0b000, 0b111, 0b000, 0b000],
            '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
            '=' => [0b000, 0b111, 0b000, 0b111, 0b000],
            '(' => [0b010, 0b100, 0b100, 0b100, 0b010],
            ')' => [0b010, 0b001, 0b001, 0b001, 0b010],
            '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
            ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
            '|' => [0b010, 0b010, 0b010, 0b010, 0b010],
            '_' => [0b000, 0b000, 0b000, 0b000, 0b111],
            ' ' => [0b000, 0b000, 0b000, 0b000, 0b000],
            // Letters (case-insensitive)
            'A' | 'a' => [0b010, 0b101, 0b111, 0b101, 0b101],
            'B' | 'b' => [0b110, 0b101, 0b110, 0b101, 0b110],
            'C' | 'c' => [0b011, 0b100, 0b100, 0b100, 0b011],
            'D' | 'd' => [0b110, 0b101, 0b101, 0b101, 0b110],
            'E' | 'e' => [0b111, 0b100, 0b110, 0b100, 0b111],
            'F' | 'f' => [0b111, 0b100, 0b110, 0b100, 0b100],
            'G' | 'g' => [0b011, 0b100, 0b101, 0b101, 0b011],
            'H' | 'h' => [0b101, 0b101, 0b111, 0b101, 0b101],
            'I' | 'i' => [0b111, 0b010, 0b010, 0b010, 0b111],
            'J' | 'j' => [0b001, 0b001, 0b001, 0b101, 0b010],
            'K' | 'k' => [0b101, 0b110, 0b100, 0b110, 0b101],
            'L' | 'l' => [0b100, 0b100, 0b100, 0b100, 0b111],
            'M' | 'm' => [0b101, 0b111, 0b111, 0b101, 0b101],
            'N' | 'n' => [0b101, 0b111, 0b111, 0b101, 0b101],
            'O' | 'o' => [0b010, 0b101, 0b101, 0b101, 0b010],
            'P' | 'p' => [0b110, 0b101, 0b110, 0b100, 0b100],
            'Q' | 'q' => [0b010, 0b101, 0b101, 0b110, 0b011],
            'R' | 'r' => [0b110, 0b101, 0b110, 0b101, 0b101],
            'S' | 's' => [0b011, 0b100, 0b010, 0b001, 0b110],
            'T' | 't' => [0b111, 0b010, 0b010, 0b010, 0b010],
            'U' | 'u' => [0b101, 0b101, 0b101, 0b101, 0b111],
            'V' | 'v' => [0b101, 0b101, 0b101, 0b101, 0b010],
            'W' | 'w' => [0b101, 0b101, 0b111, 0b111, 0b101],
            'X' | 'x' => [0b101, 0b101, 0b010, 0b101, 0b101],
            'Y' | 'y' => [0b101, 0b101, 0b010, 0b010, 0b010],
            'Z' | 'z' => [0b111, 0b001, 0b010, 0b100, 0b111],
            _ => [0b000, 0b000, 0b010, 0b000, 0b000], // fallback: small dot
        }
    };

    let scale = text_scale.max(1);
    let mut cx = x;
    for ch in text.chars() {
        let g = glyph(ch);
        for (row, &bits) in g.iter().enumerate() {
            for col in 0..3u32 {
                if (bits >> (2 - col)) & 1 == 1 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = cx + col * scale + sx;
                            let py = y + row as u32 * scale + sy;
                            if px < img.width() && py < img.height() {
                                img.put_pixel(px, py, color);
                            }
                        }
                    }
                }
            }
        }
        cx += 4 * scale; // 3 pixels + 1 gap, scaled
    }
}
