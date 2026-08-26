pub mod cli;
pub mod display;
pub mod models;
pub mod parsers;
pub mod utils;

use crate::cli::run_cli;

fn main() {
    run_cli();
}
