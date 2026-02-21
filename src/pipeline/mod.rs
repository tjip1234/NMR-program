pub mod command;
pub mod conversion;
pub mod processing;

#[cfg(test)]
mod tests {
    use super::conversion;
    use crate::log::reproducibility::ReproLog;
    use std::path::Path;

    #[test]
    fn test_load_1d_proton_jdf() {
        let jdf = Path::new("test-files/2-chlorobutane_PROTON-2-1.jdf");
        if !jdf.exists() {
            eprintln!("Skipping: test file not found");
            return;
        }
        let mut log = ReproLog::new();
        let spectrum = conversion::load_spectrum(jdf, &mut log, None)
            .expect("Failed to load PROTON JDF");

        assert!(!spectrum.real.is_empty(), "Real data should not be empty");
        assert_eq!(
            spectrum.dimensionality,
            crate::data::spectrum::Dimensionality::OneD
        );
        assert!(
            spectrum.axes[0].spectral_width_hz > 0.0,
            "Spectral width should be positive"
        );
        assert!(
            spectrum.axes[0].observe_freq_mhz > 300.0,
            "Observe freq should be > 300 MHz"
        );
        println!(
            "PROTON: {} points, SW={:.1} Hz, obs={:.3} MHz",
            spectrum.real.len(),
            spectrum.axes[0].spectral_width_hz,
            spectrum.axes[0].observe_freq_mhz,
        );
    }

    #[test]
    fn test_load_1d_carbon_jdf() {
        let jdf = Path::new("test-files/2-chlorobutane_CARBON-2-1.jdf");
        if !jdf.exists() {
            eprintln!("Skipping: test file not found");
            return;
        }
        let mut log = ReproLog::new();
        let spectrum = conversion::load_spectrum(jdf, &mut log, None)
            .expect("Failed to load CARBON JDF");

        assert!(!spectrum.real.is_empty(), "Real data should not be empty");
        println!(
            "CARBON: {} points, SW={:.1} Hz, obs={:.3} MHz",
            spectrum.real.len(),
            spectrum.axes[0].spectral_width_hz,
            spectrum.axes[0].observe_freq_mhz,
        );
    }

    #[test]
    fn test_load_2d_cosy_jdf() {
        let jdf = Path::new("test-files/2-chlorobutane_COSY-2-1.jdf");
        if !jdf.exists() {
            eprintln!("Skipping: test file not found");
            return;
        }
        let mut log = ReproLog::new();
        let spectrum = conversion::load_spectrum(jdf, &mut log, None)
            .expect("Failed to load COSY JDF");

        assert_eq!(
            spectrum.dimensionality,
            crate::data::spectrum::Dimensionality::TwoD
        );
        assert!(!spectrum.data_2d.is_empty(), "2D data should not be empty");
        assert!(spectrum.axes.len() >= 2, "Should have 2 axes");
        println!(
            "COSY: {}×{} matrix, F2 SW={:.1} Hz, F1 SW={:.1} Hz",
            spectrum.data_2d.len(),
            spectrum.data_2d.first().map(|r| r.len()).unwrap_or(0),
            spectrum.axes[0].spectral_width_hz,
            spectrum.axes[1].spectral_width_hz,
        );
    }

    #[test]
    fn test_load_2d_hsqc_jdf() {
        let jdf = Path::new("test-files/2-chlorobutane_HSQC_NUS-2-1.jdf");
        if !jdf.exists() {
            eprintln!("Skipping: test file not found");
            return;
        }
        let mut log = ReproLog::new();
        let spectrum = conversion::load_spectrum(jdf, &mut log, None)
            .expect("Failed to load HSQC JDF");

        assert_eq!(
            spectrum.dimensionality,
            crate::data::spectrum::Dimensionality::TwoD
        );
        assert!(!spectrum.data_2d.is_empty(), "2D data should not be empty");
        println!(
            "HSQC: {}×{} matrix",
            spectrum.data_2d.len(),
            spectrum.data_2d.first().map(|r| r.len()).unwrap_or(0),
        );
    }

    #[test]
    fn test_delta2pipe_found() {
        let exe = crate::data::jdf::find_delta2pipe();
        assert!(exe.is_some(), "delta2pipe should be found on this system");
        println!("delta2pipe at: {}", exe.unwrap().display());
    }
}
