use anyhow::Result;
use vergen_gix::{Build, Cargo, Emitter, Gix};

fn main() -> Result<()> {
    let build = Build::all_build();
    let cargo = Cargo::all_cargo();
    let gix = Gix::all_git();

    let mut emitter = Emitter::default();
    emitter.add_instructions(&build)?.add_instructions(&cargo)?;

    // Git metadata may fail in non-git environments (crates.io builds, tarballs).
    // Silently skip — cli.rs uses option_env! for VERGEN_GIT_DESCRIBE.
    let _ = emitter.add_instructions(&gix);

    emitter.emit()
}
