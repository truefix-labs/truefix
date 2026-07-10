use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let proto_dir = manifest_dir.join("proto");

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
    config.compile_protos(&protos, &[proto_dir])?;

    Ok(())
}
