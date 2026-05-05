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
        assert_eq!(
            spectrum.axes[0].nucleus,
            crate::data::spectrum::Nucleus::C13,
            "Carbon spectrum should have C13 nucleus, got {:?}",
            spectrum.axes[0].nucleus,
        );
        println!(
            "CARBON: {} points, SW={:.1} Hz, obs={:.3} MHz, nucleus={}",
            spectrum.real.len(),
            spectrum.axes[0].spectral_width_hz,
            spectrum.axes[0].observe_freq_mhz,
            spectrum.axes[0].nucleus,
        );
    }

    #[test]
    fn test_load_2d_cosy_jdf() {
        let jdf = Path::new("test-files/2-chlorobutane_PROTON-2-1_nmrpipe/2-chlorobutane_COSY-2-1.jdf");
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
        let ax0 = &spectrum.axes[0];
        let ax1 = &spectrum.axes[1];
        println!(
            "COSY: {}×{} matrix\n\
             F2: ref_ppm={:.4}, sw_hz={:.1}, obs_mhz={:.4}, npts={}, nucleus={}\n\
             F1: ref_ppm={:.4}, sw_hz={:.1}, obs_mhz={:.4}, npts={}, nucleus={}\n\
             F2 ppm range: {:.4} .. {:.4}\n\
             F1 ppm range: {:.4} .. {:.4}\n\
             is_freq_f2={}, is_freq_f1={}, is_frequency_domain={}",
            spectrum.data_2d.len(),
            spectrum.data_2d.first().map(|r| r.len()).unwrap_or(0),
            ax0.reference_ppm, ax0.spectral_width_hz, ax0.observe_freq_mhz, ax0.num_points, ax0.nucleus,
            ax1.reference_ppm, ax1.spectral_width_hz, ax1.observe_freq_mhz, ax1.num_points, ax1.nucleus,
            ax0.index_to_ppm(0), ax0.index_to_ppm(ax0.num_points.saturating_sub(1)),
            ax1.index_to_ppm(0), ax1.index_to_ppm(ax1.num_points.saturating_sub(1)),
            spectrum.is_freq_f2, spectrum.is_freq_f1, spectrum.is_frequency_domain,
        );
    }

    #[test]
    fn test_load_2d_hsqc_jdf() {
        let jdf = Path::new("test-files/2-chlorobutane_PROTON-2-1_nmrpipe/2-chlorobutane_HSQC_NUS-2-1.jdf");
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
        let ax0 = &spectrum.axes[0];
        let ax1 = &spectrum.axes[1];
        println!(
            "HSQC: {}×{} matrix\n\
             F2: ref_ppm={:.4}, sw_hz={:.1}, obs_mhz={:.4}, npts={}, nucleus={}\n\
             F1: ref_ppm={:.4}, sw_hz={:.1}, obs_mhz={:.4}, npts={}, nucleus={}\n\
             F2 ppm range: {:.4} .. {:.4}\n\
             F1 ppm range: {:.4} .. {:.4}\n\
             is_freq_f2={}, is_freq_f1={}, is_frequency_domain={}",
            spectrum.data_2d.len(),
            spectrum.data_2d.first().map(|r| r.len()).unwrap_or(0),
            ax0.reference_ppm, ax0.spectral_width_hz, ax0.observe_freq_mhz, ax0.num_points, ax0.nucleus,
            ax1.reference_ppm, ax1.spectral_width_hz, ax1.observe_freq_mhz, ax1.num_points, ax1.nucleus,
            ax0.index_to_ppm(0), ax0.index_to_ppm(ax0.num_points.saturating_sub(1)),
            ax1.index_to_ppm(0), ax1.index_to_ppm(ax1.num_points.saturating_sub(1)),
            spectrum.is_freq_f2, spectrum.is_freq_f1, spectrum.is_frequency_domain,
        );
    }

    #[test]
    fn test_load_2d_hmbc_jdf() {
        let jdf = Path::new("test-files/2-chlorobutane_PROTON-2-1_nmrpipe/2-chlorobutane_HMBC_NUS-2-1.jdf");
        if !jdf.exists() {
            eprintln!("Skipping: test file not found");
            return;
        }
        let mut log = ReproLog::new();
        let spectrum = conversion::load_spectrum(jdf, &mut log, None)
            .expect("Failed to load HMBC JDF");

        assert_eq!(
            spectrum.dimensionality,
            crate::data::spectrum::Dimensionality::TwoD
        );
        assert!(!spectrum.data_2d.is_empty(), "2D data should not be empty");
        let ax0 = &spectrum.axes[0];
        let ax1 = &spectrum.axes[1];
        println!(
            "HMBC: {}×{} matrix\n\
             F2: ref_ppm={:.4}, sw_hz={:.1}, obs_mhz={:.4}, npts={}, nucleus={}\n\
             F1: ref_ppm={:.4}, sw_hz={:.1}, obs_mhz={:.4}, npts={}, nucleus={}\n\
             F2 ppm range: {:.4} .. {:.4}\n\
             F1 ppm range: {:.4} .. {:.4}\n\
             is_freq_f2={}, is_freq_f1={}, is_frequency_domain={}\n\
             quad_mode={:?}, y_is_complex={}, experiment_type={:?}",
            spectrum.data_2d.len(),
            spectrum.data_2d.first().map(|r| r.len()).unwrap_or(0),
            ax0.reference_ppm, ax0.spectral_width_hz, ax0.observe_freq_mhz, ax0.num_points, ax0.nucleus,
            ax1.reference_ppm, ax1.spectral_width_hz, ax1.observe_freq_mhz, ax1.num_points, ax1.nucleus,
            ax0.index_to_ppm(0), ax0.index_to_ppm(ax0.num_points.saturating_sub(1)),
            ax1.index_to_ppm(0), ax1.index_to_ppm(ax1.num_points.saturating_sub(1)),
            spectrum.is_freq_f2, spectrum.is_freq_f1, spectrum.is_frequency_domain,
            spectrum.quad_mode, spectrum.y_is_complex, spectrum.experiment_type,
        );
    }

    #[test]
    fn test_delta2pipe_found() {
        let exe = crate::data::jdf::find_delta2pipe();
        if exe.is_none() {
            eprintln!("delta2pipe not found — skipping (not available in CI)");
            return;
        }
        println!("delta2pipe at: {}", exe.unwrap().display());
    }
}
