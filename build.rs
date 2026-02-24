use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_dir_pb = PathBuf::from(&out_dir);
    let output_file = out_dir_pb.join("libsddc.h");
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());

    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_language(cbindgen::Language::C)
        .with_include_guard("LIBSDDC_H")
        .with_cpp_compat(true)
        .with_documentation(true)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(output_file.clone());

    // Also copy the generated header to a stable location: <repo>/target/<profile>/libsddc.h
    // Derive <repo>/target/<profile> from OUT_DIR which looks like
    // <repo>/target/[<triple>/]<profile>/build/<crate-hash>/out
    let stable_dir = out_dir_pb
        .ancestors()
        .find(|p| p.file_name().map(|n| n == "build").unwrap_or(false))
        .and_then(|build_dir| build_dir.parent())
        .map(|profile_dir| profile_dir.to_path_buf())
        .unwrap_or_else(|| {
            // Fallback if OUT_DIR shape changes: honor CARGO_TARGET_DIR if set
            if let Ok(target_root) = env::var("CARGO_TARGET_DIR") {
                PathBuf::from(target_root).join(&profile)
            } else {
                PathBuf::from("target").join(&profile)
            }
        });
    let stable_file = stable_dir.join("libsddc.h");

    if let Err(e) = fs::create_dir_all(&stable_dir) {
        println!(
            "cargo:warning=failed to create target dir {}: {}",
            stable_dir.display(),
            e
        );
    } else if let Err(e) = fs::copy(&output_file, &stable_file) {
        println!(
            "cargo:warning=failed to copy header {} to {}: {}",
            output_file.display(),
            stable_file.display(),
            e
        );
    }

    // Trigger rebuild if sddc.rs changes
    println!("cargo:rerun-if-changed=src/sddc.rs");
}
