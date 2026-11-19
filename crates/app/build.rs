//! Emits the commit and build-mode metadata exposed by `/api/version`.
//!
//! `VERGEN_GIT_SHA` is honoured when set, so Docker builds can pass the commit
//! through from the bake arguments without shipping `.git` in the context.
//! `DEV_BUILD` is also allowed through so release images are not labelled as
//! local builds.

use vergen_gix::{Emitter, Gix};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=DEV_BUILD");

    let git = Gix::builder().sha(true).build();
    Emitter::new()
        .default_on_error()
        .quiet()
        .add_instructions(&git)?
        .emit()?;

    if let Ok(dev) = std::env::var("DEV_BUILD") {
        println!("cargo:rustc-env=DEV_BUILD={dev}");
    }

    Ok(())
}
