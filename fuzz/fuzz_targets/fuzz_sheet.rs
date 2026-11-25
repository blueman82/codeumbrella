//! Fuzzes the read and the check of a sheet.
#![no_main]

use codeumbrella::graph::Graph;
use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(graph) = yaml_serde::from_str::<Graph>(text) {
        // With the repository rules pages, a sheet gets to each part of the check.
        let roles = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../roles"));
        // An error is the correct answer to a bad sheet.
        let _ = graph.check(roles);
    }
});
