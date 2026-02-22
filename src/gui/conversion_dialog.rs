/// Conversion settings dialog for delta2pipe parameters
///
/// Allows users to configure all relevant delta2pipe axis parameters
/// before converting JEOL Delta .jdf files to NMRPipe format.

use serde::{Deserialize, Serialize};

/// Which method to use for reading vendor data
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ConversionMethod {
    /// Use NMRPipe converter tools (bruk2pipe, delta2pipe, var2pipe)
    NMRPipe,
    /// Use built-in native readers (no NMRPipe required)
    BuiltIn,
}

impl ConversionMethod {
    pub fn label(&self) -> &str {
        match self {
            ConversionMethod::NMRPipe => "NMRPipe tools",
            ConversionMethod::BuiltIn => "Built-in reader",
        }
    }
    pub fn short_label(&self) -> &str {
        match self {
            ConversionMethod::NMRPipe => "NMRPipe",
            ConversionMethod::BuiltIn => "Built-in",
        }
    }
}

/// Acquisition mode for delta2pipe (-xMODE / -yMODE)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AcqMode {
    Complex,    // 0
    Real,       // 1
    Sequential, // 2
}

impl AcqMode {
    pub fn label(&self) -> &str {
        match self {
            AcqMode::Complex => "Complex (States)",
            AcqMode::Real => "Real (TPPI)",
            AcqMode::Sequential => "Sequential (Bruker)",
        }
    }
    pub fn to_arg(&self) -> &str {
        match self {
            AcqMode::Complex => "Complex",
            AcqMode::Real => "Real",
            AcqMode::Sequential => "Sequential",
        }
    }
    pub fn all() -> &'static [AcqMode] {
        &[AcqMode::Complex, AcqMode::Real, AcqMode::Sequential]
    }
}

/// 2D acquisition mode (-aq2D)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Aq2D {
    Magnitude, // 0
    States,    // 2
    TPPI,      // 1
    Image,     // 3
}

impl Aq2D {
    pub fn label(&self) -> &str {
        match self {
            Aq2D::Magnitude => "Magnitude",
            Aq2D::States => "States (Complex)",
            Aq2D::TPPI => "TPPI (Real)",
            Aq2D::Image => "Image",
        }
    }
    pub fn to_arg(&self) -> &str {
        match self {
            Aq2D::Magnitude => "Magnitude",
            Aq2D::States => "States",
            Aq2D::TPPI => "TPPI",
            Aq2D::Image => "Image",
        }
    }
    pub fn all() -> &'static [Aq2D] {
        &[Aq2D::Magnitude, Aq2D::States, Aq2D::TPPI, Aq2D::Image]
    }
}

/// Digital filter handling mode
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DfMode {
    Auto,     // default
    During,   // -df
    Later,    // -nodf
    RealOnly, // -realOnly
}

impl DfMode {
    pub fn label(&self) -> &str {
        match self {
            DfMode::Auto => "Auto (default)",
            DfMode::During => "During conversion (-df)",
            DfMode::Later => "During processing (-nodf)",
            DfMode::RealOnly => "Real only, no DF (-realOnly)",
        }
    }
}

/// Per-axis conversion parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisConversionParams {
    pub override_n: bool,
    pub n: i64,          // -xN / -yN: actual size in file (real+imag pts)
    pub override_t: bool,
    pub t: i64,          // -xT / -yT: time-domain size
    pub override_sw: bool,
    pub sw: f64,         // -xSW / -ySW: spectral width Hz
    pub override_obs: bool,
    pub obs: f64,        // -xOBS / -yOBS: observe freq MHz
    pub override_car: bool,
    pub car: f64,        // -xCAR / -yCAR: carrier ppm
    pub override_mode: bool,
    pub mode: AcqMode,   // -xMODE / -yMODE
    pub override_label: bool,
    pub label: String,   // -xLAB / -yLAB
    pub override_ft: bool,
    pub ft: bool,        // -xFT / -yFT: 0=Time, 1=Freq
}

impl Default for AxisConversionParams {
    fn default() -> Self {
        Self {
            override_n: false,
            n: 0,
            override_t: false,
            t: 0,
            override_sw: false,
            sw: 0.0,
            override_obs: false,
            obs: 0.0,
            override_car: false,
            car: 0.0,
            override_mode: false,
            mode: AcqMode::Complex,
            override_label: false,
            label: String::new(),
            override_ft: false,
            ft: false,
        }
    }
}

impl AxisConversionParams {
    /// Build command-line arguments for this axis. prefix is "x" or "y".
    pub fn to_args(&self, prefix: &str) -> Vec<String> {
        let mut args = Vec::new();
        if self.override_n && self.n > 0 {
            args.push(format!("-{}N", prefix));
            args.push(self.n.to_string());
        }
        if self.override_t && self.t > 0 {
            args.push(format!("-{}T", prefix));
            args.push(self.t.to_string());
        }
        if self.override_sw && self.sw > 0.0 {
            args.push(format!("-{}SW", prefix));
            args.push(format!("{:.6}", self.sw));
        }
        if self.override_obs && self.obs > 0.0 {
            args.push(format!("-{}OBS", prefix));
            args.push(format!("{:.6}", self.obs));
        }
        if self.override_car {
            args.push(format!("-{}CAR", prefix));
            args.push(format!("{:.6}", self.car));
        }
        if self.override_mode {
            args.push(format!("-{}MODE", prefix));
            args.push(self.mode.to_arg().to_string());
        }
        if self.override_label && !self.label.is_empty() {
            args.push(format!("-{}LAB", prefix));
            args.push(self.label.clone());
        }
        if self.override_ft {
            args.push(format!("-{}FT", prefix));
            args.push(if self.ft { "Freq".to_string() } else { "Time".to_string() });
        }
        args
    }
}

/// Full conversion settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionSettings {
    pub x_axis: AxisConversionParams,
    pub y_axis: AxisConversionParams,
    pub df_mode: DfMode,
    pub override_aq2d: bool,
    pub aq2d: Aq2D,
    pub override_ndim: bool,
    pub ndim: usize,
    pub verbose: bool,
    /// Extra raw arguments the user can type in
    pub extra_args: String,
    /// Which conversion backend to use
    pub conversion_method: ConversionMethod,
}

impl Default for ConversionSettings {
    fn default() -> Self {
        Self {
            x_axis: AxisConversionParams::default(),
            y_axis: AxisConversionParams::default(),
            df_mode: DfMode::Auto,
            override_aq2d: false,
            aq2d: Aq2D::Magnitude,
            override_ndim: false,
            ndim: 1,
            verbose: true,
            extra_args: String::new(),
            conversion_method: ConversionMethod::NMRPipe,
        }
    }
}

impl ConversionSettings {
    /// Build the full list of extra arguments (beyond -in/-out/-ov).
    pub fn to_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        // Digital filter mode
        match &self.df_mode {
            DfMode::Auto => {}
            DfMode::During => args.push("-df".to_string()),
            DfMode::Later => args.push("-nodf".to_string()),
            DfMode::RealOnly => args.push("-realOnly".to_string()),
        }

        // 2D acq mode
        if self.override_aq2d {
            args.push("-aq2D".to_string());
            args.push(self.aq2d.to_arg().to_string());
        }

        // ndim
        if self.override_ndim {
            args.push("-ndim".to_string());
            args.push(self.ndim.to_string());
        }

        // Verbose
        if self.verbose {
            args.push("-verb".to_string());
        }

        // Per-axis params
        args.extend(self.x_axis.to_args("x"));
        args.extend(self.y_axis.to_args("y"));

        // Extra raw args
        if !self.extra_args.is_empty() {
            for token in self.extra_args.split_whitespace() {
                args.push(token.to_string());
            }
        }

        args
    }

    /// Generate the preview command string for display.
    pub fn preview_command(&self, exe: &str, input: &str, output: &str) -> String {
        let mut parts = vec![
            exe.to_string(),
            "-in".to_string(),
            input.to_string(),
            "-out".to_string(),
            output.to_string(),
            "-ov".to_string(),
        ];
        parts.extend(self.to_args());
        parts.join(" \\\n  ")
    }
}

/// State for the conversion settings dialog
#[derive(Debug, Clone)]
pub struct ConversionDialogState {
    pub open: bool,
    pub settings: ConversionSettings,
    /// Path waiting to be converted (set when user opens a .jdf)
    pub pending_path: Option<std::path::PathBuf>,
    /// Info text from delta2pipe -info
    pub info_text: String,
    pub info_loaded: bool,
}

impl Default for ConversionDialogState {
    fn default() -> Self {
        Self {
            open: false,
            settings: ConversionSettings::default(),
            pending_path: None,
            info_text: String::new(),
            info_loaded: false,
        }
    }
}

/// Actions from the conversion dialog
#[derive(Debug, Clone, PartialEq)]
pub enum ConversionAction {
    None,
    Convert,
    Cancel,
}

/// Show the conversion settings window. Returns action when user clicks Convert/Cancel.
pub fn show_conversion_dialog(
    ctx: &egui::Context,
    state: &mut ConversionDialogState,
) -> ConversionAction {
    let mut action = ConversionAction::None;

    if !state.open {
        return action;
    }

    let mut open = state.open;
    egui::Window::new("⚙ delta2pipe Conversion Settings")
        .open(&mut open)
        .default_size([700.0, 550.0])
        .resizable(true)
        .show(ctx, |ui| {
            let file_label = state
                .pending_path
                .as_ref()
                .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
                .unwrap_or_else(|| "—".to_string());
            ui.label(format!("File: {}", file_label));
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                // ── Info from delta2pipe ──
                ui.collapsing("ℹ File Info (delta2pipe -info)", |ui| {
                    if !state.info_loaded {
                        if ui.button("Load info from file…").clicked() {
                            if let Some(path) = &state.pending_path {
                                match crate::data::jdf::get_jdf_info(path) {
                                    Ok(info) => {
                                        state.info_text = info;
                                        state.info_loaded = true;
                                    }
                                    Err(e) => {
                                        state.info_text = format!("Error: {}", e);
                                    }
                                }
                            }
                        }
                    }
                    if !state.info_text.is_empty() {
                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .id_salt("info_scroll")
                            .show(ui, |ui| {
                                ui.style_mut().override_font_id =
                                    Some(egui::FontId::monospace(11.0));
                                ui.label(&state.info_text);
                            });
                    }
                });

                ui.add_space(4.0);

                // ── Digital Filter ──
                ui.collapsing("Digital Filter", |ui| {
                    for mode in &[DfMode::Auto, DfMode::During, DfMode::Later, DfMode::RealOnly] {
                        ui.radio_value(&mut state.settings.df_mode, mode.clone(), mode.label());
                    }
                });

                // ── General ──
                ui.collapsing("General", |ui| {
                    ui.checkbox(&mut state.settings.verbose, "Verbose output (-verb)");

                    ui.horizontal(|ui| {
                        ui.checkbox(&mut state.settings.override_ndim, "Override ndim");
                        if state.settings.override_ndim {
                            ui.add(egui::Slider::new(&mut state.settings.ndim, 1..=4).text("dims"));
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.checkbox(&mut state.settings.override_aq2d, "Override -aq2D");
                        if state.settings.override_aq2d {
                            egui::ComboBox::from_id_salt("aq2d_combo")
                                .selected_text(state.settings.aq2d.label())
                                .show_ui(ui, |ui| {
                                    for m in Aq2D::all() {
                                        ui.selectable_value(
                                            &mut state.settings.aq2d,
                                            m.clone(),
                                            m.label(),
                                        );
                                    }
                                });
                        }
                    });
                });

                // ── X-Axis ──
                ui.collapsing("X-Axis (Direct Dimension)", |ui| {
                    show_axis_params(ui, &mut state.settings.x_axis, "x");
                });

                // ── Y-Axis ──
                ui.collapsing("Y-Axis (Indirect Dimension)", |ui| {
                    show_axis_params(ui, &mut state.settings.y_axis, "y");
                });

                // ── Extra Arguments ──
                ui.collapsing("Extra Arguments (raw)", |ui| {
                    ui.label("Additional delta2pipe arguments (space-separated):");
                    ui.text_edit_singleline(&mut state.settings.extra_args);
                });

                ui.add_space(8.0);
                ui.separator();

                // ── Command Preview ──
                let exe_name = "delta2pipe";
                let input = state
                    .pending_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "<input.jdf>".to_string());
                let preview = state.settings.preview_command(exe_name, &input, "<output.fid>");
                ui.label("Command preview:");
                ui.style_mut().override_font_id = Some(egui::FontId::monospace(11.0));
                ui.label(&preview);
            });

            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("▶ Convert").clicked() {
                    action = ConversionAction::Convert;
                }
                if ui.button("Cancel").clicked() {
                    action = ConversionAction::Cancel;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Reset to defaults").clicked() {
                        let current_method = state.settings.conversion_method;
                        state.settings = ConversionSettings::default();
                        state.settings.conversion_method = current_method;
                    }
                });
            });
        });

    state.open = open;
    if !open {
        action = ConversionAction::Cancel;
    }

    action
}

/// Show editable axis parameters
fn show_axis_params(ui: &mut egui::Ui, params: &mut AxisConversionParams, prefix: &str) {
    let p = prefix.to_uppercase();

    ui.horizontal(|ui| {
        ui.checkbox(&mut params.override_n, format!("-{}N (size)", p));
        if params.override_n {
            ui.add(egui::DragValue::new(&mut params.n).speed(1).range(0..=1_000_000));
            ui.label("pts (real+imag)");
        }
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut params.override_t, format!("-{}T (TD size)", p));
        if params.override_t {
            ui.add(egui::DragValue::new(&mut params.t).speed(1).range(0..=1_000_000));
            ui.label("pts");
        }
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut params.override_sw, format!("-{}SW (sweep)", p));
        if params.override_sw {
            ui.add(egui::DragValue::new(&mut params.sw).speed(1.0).range(0.0..=1e9));
            ui.label("Hz");
        }
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut params.override_obs, format!("-{}OBS (obs freq)", p));
        if params.override_obs {
            ui.add(egui::DragValue::new(&mut params.obs).speed(0.001).range(0.0..=1500.0));
            ui.label("MHz");
        }
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut params.override_car, format!("-{}CAR (carrier)", p));
        if params.override_car {
            ui.add(egui::DragValue::new(&mut params.car).speed(0.01).range(-500.0..=500.0));
            ui.label("ppm");
        }
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut params.override_mode, format!("-{}MODE", p));
        if params.override_mode {
            egui::ComboBox::from_id_salt(format!("{}_mode_combo", prefix))
                .selected_text(params.mode.label())
                .show_ui(ui, |ui| {
                    for m in AcqMode::all() {
                        ui.selectable_value(&mut params.mode, m.clone(), m.label());
                    }
                });
        }
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut params.override_label, format!("-{}LAB", p));
        if params.override_label {
            ui.add(egui::TextEdit::singleline(&mut params.label).desired_width(60.0));
        }
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut params.override_ft, format!("-{}FT (domain)", p));
        if params.override_ft {
            ui.radio_value(&mut params.ft, false, "Time");
            ui.radio_value(&mut params.ft, true, "Freq");
        }
    });
}
