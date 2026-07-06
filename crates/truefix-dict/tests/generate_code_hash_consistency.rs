#![cfg(feature = "dict-tooling")]

use std::process::Command;

#[test]
fn non_utf8_input_hash_matches_the_runtime_lossy_parse() {
    let dir = std::env::temp_dir().join(format!("truefix-codegen-hash-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dict_path = dir.join("non-utf8.fixdict");
    let out_path = dir.join("generated.rs");

    let mut bytes = b"version FIX.TEST\n\
field 8 BeginString STRING\n\
field 9 BodyLength LENGTH\n\
field 35 MsgType STRING\n\
field 10 CheckSum STRING\n\
header 8 9 35\n\
trailer 10\n\
message 0 Heartbeat req: opt:\n\
# non-utf8: "
        .to_vec();
    bytes.extend_from_slice(&[0xff, b'\n']);
    std::fs::write(&dict_path, &bytes).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_truefix-dict"))
        .args([
            "generate-code",
            "--dict",
            dict_path.to_str().unwrap(),
            "--out",
            out_path.to_str().unwrap(),
            "--name",
            "TEST",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let generated = std::fs::read_to_string(&out_path).unwrap();
    let prefix = "pub const TEST_DICT_HASH: u64 = ";
    let generated_hash: u64 = generated
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .and_then(|value| value.strip_suffix(';'))
        .unwrap()
        .parse()
        .unwrap();
    let runtime_hash = truefix_dict::parse(&String::from_utf8_lossy(&bytes))
        .unwrap()
        .hash();

    assert_eq!(generated_hash, runtime_hash);
    let _ = std::fs::remove_dir_all(dir);
}
