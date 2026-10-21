#![no_main]

use libfuzzer_sys::fuzz_target;
use tpt_canvas::Path;

fuzz_target!(|data: &[u8]| {
    // Malformed bytes must return an error, never panic. Valid input must re-serialize.
    if let Ok(path) = Path::deserialize(data) {
        let again = Path::deserialize(&path.serialize());
        assert!(again.is_ok(), "a decoded path must round-trip");
    }
});
