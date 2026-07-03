//! `truefix-dict` — a standalone CLI for the FIX data dictionary pipeline (US13, FR-018).
//!
//! Wraps the exact same parse/codegen logic `build.rs` uses internally to build this crate's own
//! bundled dictionaries — no parallel implementation (Constitution Principle IV). Requires
//! `--features dict-tooling` (pulls in `quick-xml`, needed for `generate-dict`'s Orchestra
//! conversion).
//!
//! ```text
//! truefix-dict generate-dict --source <orchestra.xml> --out <normalized.fixdict>
//! truefix-dict generate-code --dict <normalized.fixdict> --out <generated.rs> [--name <Name>]
//! truefix-dict validate --dict <normalized.fixdict>
//! ```
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

use std::collections::BTreeMap;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let Some((subcommand, rest)) = args.split_first() else {
        print_usage();
        return Err("no subcommand given".to_owned());
    };
    match subcommand.as_str() {
        "generate-dict" => generate_dict(rest),
        "generate-code" => generate_code(rest),
        "validate" => validate(rest),
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(())
        }
        other => Err(format!(
            "unknown subcommand {other:?} (expected generate-dict, generate-code, or validate; \
             see --help)"
        )),
    }
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  truefix-dict generate-dict --source <orchestra.xml> --out <normalized.fixdict>");
    eprintln!(
        "  truefix-dict generate-code --dict <normalized.fixdict> --out <generated.rs> [--name <Name>]"
    );
    eprintln!("  truefix-dict validate --dict <normalized.fixdict>");
}

/// Parse `--key value` pairs from `args` into a map (last occurrence of a repeated key wins).
fn parse_flags(args: &[String]) -> Result<BTreeMap<String, String>, String> {
    let mut flags = BTreeMap::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let Some(key) = arg.strip_prefix("--") else {
            return Err(format!(
                "unexpected argument {arg:?} (expected --flag value)"
            ));
        };
        let value = it
            .next()
            .ok_or_else(|| format!("--{key} requires a value"))?;
        flags.insert(key.to_owned(), value.clone());
    }
    Ok(flags)
}

fn required_flag<'a>(flags: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, String> {
    flags
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("--{key} is required"))
}

fn generate_dict(args: &[String]) -> Result<(), String> {
    let flags = parse_flags(args)?;
    let source = required_flag(&flags, "source")?;
    let out = required_flag(&flags, "out")?;
    let xml = std::fs::read_to_string(source).map_err(|e| format!("reading {source}: {e}"))?;
    // `--format qfj` (US9, feature 005, FR-031): the classic QuickFIX DTD-based dictionary
    // schema, as opposed to the default `orchestra` (FIX Orchestra repository XML). Requires
    // `--version` (the target `.fixdict` version directive, e.g. "FIX.4.4") since QuickFIX's XML
    // carries major/minor/servicepack as separate attributes rather than one version string.
    let dict_text = match flags.get("format").map(String::as_str) {
        Some("qfj") => {
            let version = required_flag(&flags, "version")?;
            truefix_dict::qfj_xml::convert(&xml, version)
                .map_err(|e| format!("converting {source}: {e}"))?
        }
        None | Some("orchestra") => truefix_dict::orchestra::convert(&xml)
            .map_err(|e| format!("converting {source}: {e}"))?,
        Some(other) => {
            return Err(format!(
                "unknown --format {other:?} (expected orchestra or qfj)"
            ))
        }
    };
    std::fs::write(out, &dict_text).map_err(|e| format!("writing {out}: {e}"))?;
    println!("wrote {out} ({} bytes)", dict_text.len());
    Ok(())
}

fn generate_code(args: &[String]) -> Result<(), String> {
    let flags = parse_flags(args)?;
    let dict_path = required_flag(&flags, "dict")?;
    let out = required_flag(&flags, "out")?;
    let bytes = std::fs::read(dict_path).map_err(|e| format!("reading {dict_path}: {e}"))?;
    let text = String::from_utf8_lossy(&bytes);
    let dict = truefix_dict::parse(&text).map_err(|e| format!("parsing {dict_path}: {e}"))?;
    let name = match flags.get("name") {
        Some(n) => n.clone(),
        None => dict
            .version()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_uppercase(),
    };
    let code = truefix_dict::codegen::generate(&name, &bytes)
        .map_err(|e| format!("generating code for {dict_path}: {e}"))?;
    std::fs::write(out, &code).map_err(|e| format!("writing {out}: {e}"))?;
    println!("wrote {out} ({} bytes) for module {name}", code.len());
    Ok(())
}

fn validate(args: &[String]) -> Result<(), String> {
    let flags = parse_flags(args)?;
    let dict_path = required_flag(&flags, "dict")?;
    let text =
        std::fs::read_to_string(dict_path).map_err(|e| format!("reading {dict_path}: {e}"))?;
    let dict = truefix_dict::parse(&text).map_err(|e| format!("{dict_path}: {e}"))?;
    println!(
        "{dict_path}: OK — version={} fields={} messages={} hash={}",
        dict.version(),
        dict.field_count(),
        dict.message_count(),
        dict.hash()
    );
    Ok(())
}
