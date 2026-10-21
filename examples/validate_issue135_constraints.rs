//! Strictly decode native constraint groups and exercise DWG/DXF codecs.
//!
//! cargo run --example validate_issue135_constraints -- INPUT.dwg OUTPUT_DIR
//! cargo run --example validate_issue135_constraints -- --compare INPUT.dwg RESULT.dwg
#[path = "../tests/support/issue135_validation.rs"]
mod validation;

use std::path::{Path, PathBuf};

use acadrust::{DwgWriter, DxfWriter};
use validation::{
    compare_groups, constraint_groups, disable_record_passthrough, print_summary, read_strict,
    ValidationResult,
};

fn validate() -> ValidationResult<()> {
    // This executable owns its environment; cached records must never mask a codec defect.
    std::env::remove_var("ACADRUST_RAW_ALL");
    std::env::remove_var("ACADRUST_RAW_EXCLUDE");
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--compare") {
        if args.len() != 3 {
            return Err("usage: validate_issue135_constraints --compare INPUT RESULT".into());
        }
        let expected = read_strict(Path::new(&args[1]))?;
        let actual = read_strict(Path::new(&args[2]))?;
        print_summary(Path::new(&args[2]), &actual)?;
        let mut expected = constraint_groups(&expected)?;
        let mut actual = constraint_groups(&actual)?;
        // AutoCAD may reorder the owned-action set on save. Nodes refer to
        // dependencies by handle, so this order carries no graph semantics.
        for group in expected.values_mut().chain(actual.values_mut()) {
            group.actions.sort();
        }
        compare_groups(&expected, &actual)?;
        println!("All constraint group semantics match.");
        return Ok(());
    }
    if args.len() != 2 {
        return Err("usage: validate_issue135_constraints INPUT.dwg OUTPUT_DIR".into());
    }
    let input = Path::new(&args[0]);
    let output = PathBuf::from(&args[1]);
    std::fs::create_dir_all(&output)?;
    let mut document = read_strict(input)?;
    print_summary(input, &document)?;
    let expected = constraint_groups(&document)?;
    disable_record_passthrough(&mut document);
    let stem = input
        .file_stem()
        .ok_or("input needs a filename")?
        .to_string_lossy();
    let dwg = output.join(format!("{stem}_rust.dwg"));
    let dxf = output.join(format!("{stem}_rust.dxf"));
    let binary_dxf = output.join(format!("{stem}_rust_binary.dxf"));
    let through_dxf = output.join(format!("{stem}_rust_via_dxf.dwg"));
    let through_binary_dxf = output.join(format!("{stem}_rust_via_binary_dxf.dwg"));
    DwgWriter::write_to_file(&dwg, &document)?;
    DxfWriter::new(&document).write_to_file(&dxf)?;
    DxfWriter::new_binary(&document).write_to_file(&binary_dxf)?;
    let decoded_dxf = read_strict(&dxf)?;
    DwgWriter::write_to_file(&through_dxf, &decoded_dxf)?;
    let decoded_binary_dxf = read_strict(&binary_dxf)?;
    DwgWriter::write_to_file(&through_binary_dxf, &decoded_binary_dxf)?;
    let mut failures = Vec::new();
    for path in [&dwg, &dxf, &binary_dxf, &through_dxf, &through_binary_dxf] {
        let result = (|| -> ValidationResult<()> {
            let actual = read_strict(path)?;
            print_summary(path, &actual)?;
            compare_groups(&expected, &constraint_groups(&actual)?)
        })();
        match result {
            Ok(()) => println!("PASS {}: constraint group semantics match", path.display()),
            Err(error) => failures.push(format!("{}: {error}", path.display())),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n").into())
    }
}

fn main() {
    if let Err(error) = validate() {
        eprintln!("FAIL: {error}");
        std::process::exit(1);
    }
}
