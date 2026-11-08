//! The balance harness's command line (`docs/design/balance-harness.md`, section 8; #537).
//!
//!     cargo run --release -p fn_harness --bin balance -- verdict --seeds 1-4
//!
//! The commands and their options are in `fn_harness::cli`.

fn main() -> std::process::ExitCode {
    fn_harness::cli::main(std::env::args().skip(1).collect())
}
