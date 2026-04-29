use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

use crate::core::{clean, detector, doctor, inspect, runner};

#[derive(Parser)]
#[command(name = "capsule", version, about = "Universal zero-setup dev runtime.")]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run(RunArgs),
    Scan(PathArg),
    Inspect(PathArg),
    Lock(RunArgs),
    Doctor,
    Clean(CleanArgs),
}

#[derive(Args, Clone)]
pub struct RunArgs {
    pub path: PathBuf,
    #[arg(long, short = 'y')]
    pub yes: bool,
    #[arg(long)]
    pub no_install: bool,
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Args)]
struct PathArg {
    path: PathBuf,
}

#[derive(Args)]
struct CleanArgs {
    #[arg(long)]
    python: bool,
    #[arg(long)]
    node: bool,
    #[arg(long)]
    all: bool,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run(args) => {
            runner::run_path(&args.path, args.yes, args.no_install, args.verbose)
        }
        Commands::Scan(args) => {
            let project = detector::detect(&args.path)?;
            runner::scan_path(&project)
        }
        Commands::Inspect(args) => inspect::inspect_path(&args.path),
        Commands::Lock(args) => {
            runner::lock_path(&args.path, args.yes, args.no_install, args.verbose)
        }
        Commands::Doctor => doctor::doctor(),
        Commands::Clean(args) => clean::clean(args.python, args.node, args.all),
    }
}
