//! Dead Sea Scrolls Hebrew search: one program, no installation.
//!
//! Run it with no arguments to start the page in your browser. `dss-search stats ...` gives the
//! same counts as the Statistics tab on the command line.

mod assets;
mod commands;
#[cfg(feature = "download")]
mod download;
mod server;

use assets::Sources;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

const ABOUT: &str = "Search, read and analyse the Hebrew and Aramaic text of the Dead Sea Scrolls. \
Runs on your own computer and works offline. With no command it starts the search page and opens it in your browser.";

const EXAMPLES: &str = "Examples:
  dss-search                                   start the page and open the browser
  dss-search --port 9000 --no-browser          choose the port, do not open a browser
  dss-search stats info                        what is in the corpus
  dss-search stats freq --top 30 --exclude \"ם א\"
  dss-search stats dist יחד --by cave          where a word occurs
  dss-search stats colloc יחד --window 4       words that occur near it
  dss-search stats keyness --target 1QS        words typical of the Community Rule
  dss-search stats export words --out words.csv  the whole corpus as a table
  dss-search build --tf-dir ../dss/tf/2.0.1    rebuild the data (only to update it)

The program is under the MIT license. The text built into it is the ETCBC/CACCHT Dead Sea
Scrolls dataset, CC BY-NC 4.0: free to share with attribution, not for commercial use.";

#[derive(Parser)]
#[command(name = "dss-search", version, about = ABOUT, after_help = EXAMPLES)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[command(flatten)]
    serve: ServeOptions,
    /// Use dss.json, bible.json and scroll-names.json from this folder instead of the built-in copies
    #[arg(long, global = true, value_name = "DIR")]
    data_dir: Option<PathBuf>,
}

#[derive(Args, Clone, Default)]
pub struct ServeOptions {
    /// Port to listen on (default: 8000, or the next free one)
    #[arg(long)]
    pub port: Option<u16>,
    /// Do not open a browser window
    #[arg(long)]
    pub no_browser: bool,
    /// Use index.html, style.css, app.js and search.js from this folder instead of the built-in copies
    #[arg(long, value_name = "DIR")]
    pub web_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Start the search page on this computer (this is what happens with no command)
    Serve(ServeOptions),
    /// Rebuild data/dss.json and data/bible.json from the ETCBC/dss dataset (only to update the data)
    Build(commands::BuildArgs),
    /// Counts and comparisons on the command line (the same as the Statistics tab)
    Stats {
        #[command(subcommand)]
        command: commands::StatsCommand,
    },
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Some(Command::Stats { command }) => {
            let sources = Sources { web_dir: None, data_dir: cli.data_dir };
            commands::run_stats(&sources, command)
        }
        Some(Command::Build(args)) => commands::run_build(args),
        Some(Command::Serve(options)) => serve(options, cli.data_dir),
        None => serve(cli.serve, cli.data_dir),
    };
    if let Err(message) = result {
        eprintln!("Error: {message}");
        std::process::exit(2);
    }
}

fn serve(options: ServeOptions, data_dir: Option<PathBuf>) -> Result<(), String> {
    let sources = Sources { web_dir: options.web_dir, data_dir };
    server::run(sources, options.port, !options.no_browser)
}
