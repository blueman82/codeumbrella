//! Fuzzes the token count of the trace of a Codex worker.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        codeumbrella::codex::count_tokens(text, &mut Default::default());
    }
});
