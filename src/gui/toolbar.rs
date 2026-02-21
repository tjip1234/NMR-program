/// Toolbar — top menu bar with file operations and quick actions

use std::path::PathBuf;

/// Actions that can be triggered from the toolbar
#[derive(Debug, Clone, PartialEq)]
pub enum ToolbarAction {
    None,
    OpenFile,
    OpenFolder,
    SaveProject,
    LoadProject,
    ExportImage,
    ExportData,
    ExportLog,
    Undo,
    Redo,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ThemeToggle,
    ShowAbout,
}

/// Render the toolbar and return any triggered action
pub fn show_toolbar(ctx: &egui::Context, theme_label: &str) -> ToolbarAction {
    let mut action = ToolbarAction::None;

    egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            // File menu
            ui.menu_button("📁 File", |ui| {
                if ui.button("📂 Open File…").clicked() {
                    action = ToolbarAction::OpenFile;
                    ui.close_menu();
                }
                if ui.button("📁 Open Folder…").clicked() {
                    action = ToolbarAction::OpenFolder;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("💾 Save Project…").clicked() {
                    action = ToolbarAction::SaveProject;
                    ui.close_menu();
                }
                if ui.button("📂 Load Project…").clicked() {
                    action = ToolbarAction::LoadProject;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("🖼 Export Image…").clicked() {
                    action = ToolbarAction::ExportImage;
                    ui.close_menu();
                }
                if ui.button("� Export Data…").clicked() {
                    action = ToolbarAction::ExportData;
                    ui.close_menu();
                }
                if ui.button("�📋 Export Log…").clicked() {
                    action = ToolbarAction::ExportLog;
                    ui.close_menu();
                }
            });

            // Edit menu
            ui.menu_button("✏️ Edit", |ui| {
                if ui.button("↩ Undo").clicked() {
                    action = ToolbarAction::Undo;
                    ui.close_menu();
                }
                if ui.button("↪ Redo").clicked() {
                    action = ToolbarAction::Redo;
                    ui.close_menu();
                }
            });

            // View menu
            ui.menu_button("🔍 View", |ui| {
                if ui.button("🔍+ Zoom In").clicked() {
                    action = ToolbarAction::ZoomIn;
                    ui.close_menu();
                }
                if ui.button("🔍− Zoom Out").clicked() {
                    action = ToolbarAction::ZoomOut;
                    ui.close_menu();
                }
                if ui.button("🔄 Reset Zoom").clicked() {
                    action = ToolbarAction::ZoomReset;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(format!("🎨 Theme: {}", theme_label)).clicked() {
                    action = ToolbarAction::ThemeToggle;
                    ui.close_menu();
                }
            });

            // Help menu
            ui.menu_button("❓ Help", |ui| {
                if ui.button("ℹ About").clicked() {
                    action = ToolbarAction::ShowAbout;
                    ui.close_menu();
                }
            });

            // Spacer + quick theme toggle
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Theme quick-toggle button
                if ui.add(egui::Button::new(
                    egui::RichText::new(theme_label).size(12.0)
                ).corner_radius(12.0)).clicked() {
                    action = ToolbarAction::ThemeToggle;
                }
                ui.separator();
                ui.label(
                    egui::RichText::new("NMR Spectral Processing")
                        .color(egui::Color32::from_rgb(0x70, 0x75, 0x80))
                        .size(12.0),
                );
            });
        });
    });

    action
}

/// Show file-open dialog for NMR files
pub fn open_file_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Open NMR Data File")
        .add_filter("JEOL Delta", &["jdf"])
        .add_filter("NMRPipe", &["fid", "ft1", "ft2"])
        .add_filter("All Files", &["*"])
        .pick_file()
}

/// Show folder picker dialog
pub fn open_folder_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Open NMR Data Directory")
        .pick_folder()
}

/// Show save dialog for image export
pub fn save_image_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Export Spectrum Image")
        .add_filter("PNG Image", &["png"])
        .add_filter("SVG Image", &["svg"])
        .save_file()
}

/// Show save dialog for data export (peak list, integrals, etc.)
pub fn save_data_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Export Peak / Integration Data")
        .add_filter("CSV (comma-separated)", &["csv"])
        .add_filter("TSV (tab-separated)", &["tsv"])
        .add_filter("Text File", &["txt"])
        .save_file()
}

/// Show save dialog for log export
pub fn save_log_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Export Processing Log")
        .add_filter("Text File", &["txt"])
        .add_filter("JSON", &["json"])
        .add_filter("Shell Script", &["sh"])
        .save_file()
}
