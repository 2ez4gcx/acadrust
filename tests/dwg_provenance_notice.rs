const NOTICES: &str = include_str!("../THIRD_PARTY_NOTICES.md");

#[test]
fn includes_acadsharp_mit_attribution_and_license_terms() {
    for required in [
        "ACadSharp",
        "Copyright (c) 2021 Albert Domenech",
        "Permission is hereby granted, free of charge",
        "The above copyright notice and this permission notice",
        "THE SOFTWARE IS PROVIDED \"AS IS\"",
    ] {
        assert!(NOTICES.contains(required), "missing notice text: {required}");
    }
}

#[test]
fn records_oda_and_reverse_engineering_as_primary_format_sources() {
    for required in [
        "Open Design Alliance (ODA)",
        "specification and reverse engineering",
        "reverse engineering",
        "LibreDWG",
        "src/reedsolomon.c",
        "those values are test data",
        "translated routines.",
        "no LibreDWG routines",
    ] {
        assert!(NOTICES.contains(required), "missing provenance text: {required}");
    }
}
