use clap::Parser;

use mint_faa::cli::Cli;

fn main() {
    let cli = Cli::parse();
    if let Err(e) = cli.run() {
        eprintln!("mint: error: {e}");
        std::process::exit(1);
    }
}
