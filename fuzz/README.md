# Fuzzing

Targets live in `crates/prism-fuzz/src/lib.rs` (OPC container, DOCX, XLSX, PPTX,
formula text, number formats, DrawingML XML, preset geometry). Two drivers:

**Stable (any machine, also CI):** a built-in mutation fuzzer. Office files are
mutated inside the ZIP and re-zipped so inputs reach the XML readers.

```bash
cargo run -p prism-fuzz --release -- all 600
```

Arguments: `<target|all> [seconds] [seed]`. Panicking inputs are saved to
`target/fuzz-artifacts/`; reproduce with
`cargo run -p prism-fuzz --release -- repro <target> <file>`. Once fixed, copy
the input to `crates/prism-fuzz/regressions/<target>-<hash>.bin` — `cargo test`
replays every file there.

**libFuzzer (coverage-guided, nightly):**

```bash
cargo install cargo-fuzz
cd fuzz && cargo +nightly fuzz run pptx -- -max_total_time=600
```
