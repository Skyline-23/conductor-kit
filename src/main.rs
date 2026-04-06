mod cli;
mod runtime;

use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    if let Err(err) = cli::execute::execute_command(&args[1..]) {
        eprintln!("{err}");
        process::exit(1);
    }
}
