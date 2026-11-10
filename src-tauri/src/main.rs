// Release builds run without a console window on Windows; the command line borrows the caller's (src/cli.rs).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if switchgit_lib::cli::wants_cli(&args) {
        std::process::exit(switchgit_lib::cli::run(&args));
    }
    switchgit_lib::run()
}
