//! Smoke tests for all preset configs.
//! Quick tests verify that phasma starts successfully with each config.
//! Full tests (ignored by default) run the simulation for up to 5 minutes.

use std::process::{Command, Stdio};
use std::time::Duration;

fn run_smoke(name: &str, timeout: Duration) {
    let config_path = format!("{}/configs/{name}.toml", env!("CARGO_MANIFEST_DIR"));
    assert!(
        std::path::Path::new(&config_path).exists(),
        "Config file not found: {config_path}"
    );

    let tmpdir = tempfile::tempdir().expect("failed to create temp dir");

    let mut child = Command::new(env!("CARGO_BIN_EXE_phasma"))
        .args(["--config", &config_path, "--batch"])
        .current_dir(tmpdir.path())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn phasma");

    let poll_interval = Duration::from_millis(50);
    let start = std::time::Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    let stderr = child.stderr.take().map(|mut s| {
                        let mut buf = String::new();
                        std::io::Read::read_to_string(&mut s, &mut buf).ok();
                        buf
                    }).unwrap_or_default();
                    panic!(
                        "phasma exited with {status} for config '{name}':\n{stderr}"
                    );
                }
                return; // exited successfully
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    // Still running after timeout — init succeeded, kill and pass
                    let _ = child.kill();
                    let _ = child.wait();
                    return;
                }
                std::thread::sleep(poll_interval);
            }
            Err(e) => panic!("error waiting for phasma: {e}"),
        }
    }
}

macro_rules! smoke_test {
    ($name:ident) => {
        #[test]
        fn $name() {
            run_smoke(stringify!($name), Duration::from_secs(1));
        }
    };
}

macro_rules! full_test {
    ($name:ident) => {
        #[test]
        #[ignore]
        fn $name() {
            run_smoke(stringify!($name), Duration::from_secs(300));
        }
    };
}

mod quick {
    use super::*;

    smoke_test!(debug);
    smoke_test!(plummer);
    smoke_test!(plummer_hires);
    smoke_test!(plummer_yoshida);
    smoke_test!(plummer_unsplit);
    smoke_test!(plummer_lomac);
    smoke_test!(plummer_tt);
    smoke_test!(plummer_spectral);
    smoke_test!(plummer_multigrid);
    smoke_test!(plummer_spherical);
    smoke_test!(plummer_tensor_poisson);
    smoke_test!(hernquist);
    smoke_test!(king);
    smoke_test!(nfw);
    smoke_test!(nfw_tree);
    smoke_test!(zeldovich);
    smoke_test!(disk_bar);
    smoke_test!(merger_equal);
    smoke_test!(merger_unequal);
    smoke_test!(tidal_point);
    smoke_test!(tidal_nfw);
    smoke_test!(jeans_unstable);
    smoke_test!(jeans_stable);
    smoke_test!(plummer_128);
    smoke_test!(plummer_ht);
}

mod full {
    use super::*;

    full_test!(debug);
    full_test!(plummer);
    full_test!(plummer_hires);
    full_test!(plummer_yoshida);
    full_test!(plummer_unsplit);
    full_test!(plummer_lomac);
    full_test!(plummer_tt);
    full_test!(plummer_spectral);
    full_test!(plummer_multigrid);
    full_test!(plummer_spherical);
    full_test!(plummer_tensor_poisson);
    full_test!(hernquist);
    full_test!(king);
    full_test!(nfw);
    full_test!(nfw_tree);
    full_test!(zeldovich);
    full_test!(disk_bar);
    full_test!(merger_equal);
    full_test!(merger_unequal);
    full_test!(tidal_point);
    full_test!(tidal_nfw);
    full_test!(jeans_unstable);
    full_test!(jeans_stable);
    full_test!(plummer_128);
    full_test!(plummer_ht);
}
