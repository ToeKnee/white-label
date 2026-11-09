//! Regenerates `public/main.css` from the tailwind + daisyUI front end, so
//! the stylesheet can never lag behind the classes written in `src/` and
//! `style/`.
//!
//! Tailwind generates utilities by scanning the Rust sources, which means a
//! `main.css` built before some new utility classes appeared renders those
//! layouts unstyled. This step reruns `npm run css` whenever `src/`,
//! `style/` or the npm config changes, so every `cargo`/`dx` build carries a
//! stylesheet that matches the code it compiles.
//!
//! The tailwind CLI lives in `node_modules` (a one-shot run, no network), so
//! machines that build only the Rust side — a deployment box without a node
//! toolchain — skip the step with a loud warning instead of failing the
//! build. When the toolchain is there but the run fails, the build fails too:
//! a broken stylesheet should not ship quietly.

use std::env;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

/// Inputs whose change makes the generated stylesheet stale. `public/` is
/// deliberately absent: this step writes `public/main.css`, and watching it
/// would loop the build.
const REBUILD_INPUTS: &[&str] = &[
    "build.rs",
    "package.json",
    "package-lock.json",
    "tailwind.config.js",
    "style",
    "src",
];

fn main() -> ExitCode {
    for input in REBUILD_INPUTS {
        println!("cargo:rerun-if-changed={input}");
    }

    let manifest_dir = match env::var("CARGO_MANIFEST_DIR") {
        Ok(dir) => dir,
        Err(missing) => {
            println!("cargo:warning=CARGO_MANIFEST_DIR unset ({missing}); skipping the css step");
            return ExitCode::SUCCESS;
        }
    };
    let manifest = Path::new(&manifest_dir);

    // Without the npm front end (a bare crate copy) or without `npm install`
    // having run, the stylesheet cannot be rebuilt here — warn and continue.
    if !manifest.join("package.json").is_file() {
        println!(
            "cargo:warning=no package.json under {}; skipping the css step",
            manifest.display()
        );
        return ExitCode::SUCCESS;
    }
    if !manifest.join("node_modules").is_dir() {
        println!(
            "cargo:warning=node_modules missing under {}; run `npm install` to build \
             public/main.css — skipping the css step",
            manifest.display()
        );
        return ExitCode::SUCCESS;
    }

    let css_build = Command::new("npm")
        .args(["run", "css"])
        .current_dir(manifest)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    match css_build {
        Ok(built) if built.status.success() => ExitCode::SUCCESS,
        Ok(built) => {
            // npm already explains what tailwind choked on.
            println!(
                "cargo::error=`npm run css` failed: {}",
                String::from_utf8_lossy(&built.stderr).trim()
            );
            ExitCode::FAILURE
        }
        Err(npm_missing) => {
            println!(
                "cargo:warning=npm not on PATH ({npm_missing}); public/main.css may be \
                 stale — run `npm run css` before serving this build"
            );
            ExitCode::SUCCESS
        }
    }
}
