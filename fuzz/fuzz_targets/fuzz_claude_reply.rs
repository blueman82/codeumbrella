//! Fuzzes the read of the reply of a Claude worker.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        // An error is the correct answer to a bad reply.
        let _ = codeumbrella::claude::read_reply(text, &mut Default::default());
    }
});
