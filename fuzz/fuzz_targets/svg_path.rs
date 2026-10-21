#![no_main]

use libfuzzer_sys::fuzz_target;
use tpt_canvas::Path;

fuzz_target!(|data: &[u8]| {
    // Non-UTF-8 input is not a path string, so it is skipped rather than parsed.
    if let Ok(text) = core::str::from_utf8(data) {
        let _ = Path::from_svg_d(text);
    }
});
