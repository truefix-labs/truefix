use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let proto_dir = manifest_dir.join("proto");
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    println!("cargo:rerun-if-changed={}", proto_dir.display());

    let mut protos = Vec::new();
    for entry in fs::read_dir(&proto_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "proto")
        {
            println!("cargo:rerun-if-changed={}", path.display());
            protos.push(path);
        }
    }
    protos.sort();

    let mut config = prost_build::Config::new();
    config.bytes(["."]);
    config.compile_well_known_types();
    config.compile_protos(&protos, &[&proto_dir])?;

    // Collect all generated .rs files and write a single include file.
    let mut modules = Vec::new();
    for entry in fs::read_dir(&out_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "rs") {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                == Some("pb_modules.rs")
            {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                modules.push(stem.to_owned());
            }
        }
    }
    modules.sort();

    let mut includes = String::new();
    for module in &modules {
        includes.push_str(&format!(
            "pub mod {module} {{ include!(concat!(env!(\"OUT_DIR\"), \"/{module}.rs\")); }}\n"
        ));
    }

    fs::write(out_dir.join("pb_modules.rs"), includes)?;

    Ok(())
}
