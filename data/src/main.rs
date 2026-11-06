//! Dead Sea Scrolls Hebrew search: one program, no installation.
//!
//! Run it with no arguments to open the search window. `dss-search stats ...` gives the same
//! counts as the Statistics tab on the command line.

mod assets;
mod commands;
#[cfg(feature = "download")]
mod download;
mod server;
#[cfg(feature = "window")]
mod window;

use assets::Sources;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

const ABOUT: &str = "Search, read and analyse the Hebrew and Aramaic text of the Dead Sea Scrolls. \
Runs on your own computer and works offline. With no command it opens the search window.";

const EXAMPLES: &str = "Examples:
  dss-search                                   open the search window
  dss-search stats info                        what is in the corpus
  dss-search stats freq --top 30 --exclude \"ם א\"
  dss-search stats dist יחד --by cave          where a word occurs
  dss-search stats colloc יחד --window 4       words that occur near it
  dss-search stats keyness --target 1QS        words typical of the Community Rule
  dss-search stats export words --out words.csv  the whole corpus as a table
  dss-search serve --port 9000                 no window: serve the page to a browser instead
  dss-search build --tf-dir ../dss/tf/2.0.1    rebuild the data (only to update it)

The program is under the MIT license. The text built into it is the ETCBC/CACCHT Dead Sea
Scrolls dataset, CC BY-NC 4.0: free to share with attribution, not for commercial use.";

#[derive(Parser)]
#[command(name = "dss-search", version, about = ABOUT, after_help = EXAMPLES)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Use dss.json, bible.json and scroll-names.json from this folder instead of the built-in copies
    #[arg(long, global = true, value_name = "DIR")]
    data_dir: Option<PathBuf>,
    /// Use index.html, style.css, app.js and search.js from this folder instead of the built-in copies
    #[arg(long, global = true, value_name = "DIR")]
    web_dir: Option<PathBuf>,
    /// Open the window, check that the page works in it, and exit (used by automatic checks)
    #[arg(long, hide = true)]
    check_window: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Serve the page on this computer without a window, to open in a browser (for developers and servers)
    Serve {
        /// Port to listen on (default: 8000, or the next free one)
        #[arg(long)]
        port: Option<u16>,
    },
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
    let sources = Sources { web_dir: cli.web_dir, data_dir: cli.data_dir };
    let result = match cli.command {
        Some(Command::Stats { command }) => commands::run_stats(&sources, command),
        Some(Command::Build(args)) => commands::run_build(args),
        Some(Command::Serve { port }) => server::run(sources, port),
        None => open_window(sources, cli.check_window),
    };
    if let Err(message) = result {
        eprintln!("Error: {message}");
        std::process::exit(2);
    }
}

#[cfg(feature = "window")]
fn open_window(sources: Sources, check: bool) -> Result<(), String> {
    window::run(sources, check)
}

#[cfg(not(feature = "window"))]
fn open_window(_sources: Sources, _check: bool) -> Result<(), String> {
    Err("This copy of the program was built without a window. Start it with `dss-search serve` and open the address it prints in a browser, or use the commands (see --help).".to_string())
}
