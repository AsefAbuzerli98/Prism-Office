#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| prism_fuzz::numfmt(data));
