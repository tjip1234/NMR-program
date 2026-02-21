# GEMINI context for nmr_gui

## Project Overview
`nmr_gui` is a modern, high-performance GUI application written in Rust for NMR (Nuclear Magnetic Resonance) spectral processing. It provides a user-friendly interface for common NMR operations while ensuring scientific reproducibility through a mandatory logging system that records every processing step.

### Key Technologies
- **Language:** Rust (Edition 2021)
- **GUI Framework:** `egui` / `eframe` (Immediate mode UI)
- **Plotting:** `egui_plot` for interactive 1D/2D spectrum visualization
- **NMR Backend:** Integrates with **NMRPipe** via subprocesses but includes native Rust fallbacks for all core processing operations.
- **FFT:** `rustfft` for frequency-domain transformations.
- **Data Handling:** Native JEOL Delta (`.jdf`) reader and NMRPipe format support.
- **Export:** `image` crate for publication-quality PNG/SVG exports.

### Architecture
The project follows a clean modular structure:
- **`src/app.rs`**: Central state management (`NmrApp`) and main UI loop. Handles undo/redo, file loading, and coordination between components.
- **`src/data/`**: Core data structures. `spectrum.rs` defines `SpectrumData` and `AxisParams`.
- **`src/pipeline/`**: Processing engine. `processing.rs` contains the mathematical implementations for FT, phase correction, apodization, etc. `conversion.rs` handles vendor format detection and conversion.
- **`src/gui/`**: Modular UI components (toolbar, sidebar pipeline panel, spectrum viewers, and interactive dialogs).
- **`src/log/`**: `reproducibility.rs` implements the logging system that tracks every operation and generates reproducible shell scripts.

## Building and Running
The project uses standard Cargo commands:

- **Build:** `cargo build` (debug) or `cargo build --release` (optimized).
- **Run:** `cargo run --release`.
- **Test:** `cargo test` (includes unit tests for logging and processing).

### Dependencies
Requires **NMRPipe** to be installed and in the system `PATH` for full functionality (e.g., Bruker/Varian conversion). If NMRPipe is missing, the application defaults to "Built-in mode" using native Rust implementations.

## Development Conventions
- **State Management:** Uses immediate-mode UI patterns. State is centralized in `NmrApp` and passed down to component functions.
- **Reproducibility:** Every new processing operation **must** be recorded in the `ReproLog`. Any modification to `SpectrumData` should be accompanied by a log entry.
- **Undo/Redo:** Implemented via full state snapshots in `NmrApp.undo_stack`. New operations should call `push_undo` before execution.
- **Error Handling:** Prefer returning `Result` and displaying status messages in the GUI's status bar via `self.status_message`.
- **NMR Convention:** High ppm (downfield) is displayed on the left of the x-axis.

## Key Files
- `src/data/spectrum.rs`: The "Source of Truth" for NMR data structures.
- `src/pipeline/processing.rs`: Where the math happens (FT, Phasing, etc.).
- `src/log/reproducibility.rs`: Manages the operation history and script generation.
- `src/gui/spectrum_view.rs`: Custom plotting logic for 1D NMR data.
- `src/gui/contour_view.rs`: Contour plotting logic for 2D NMR data.
