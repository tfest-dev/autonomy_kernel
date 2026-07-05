mod cli;
mod output;

use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match cli::run_cli(args.iter().map(String::as_str)) {
        Ok(output) => {
            print!("{output}");
            process::exit(cli::EXIT_SUCCESS);
        }
        Err(error) => {
            eprintln!("{}", output::render_error(&error));
            process::exit(error.exit_code());
        }
    }
}
