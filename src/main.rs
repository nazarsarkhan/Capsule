mod adapters;
mod cli;
mod core;
mod resolver;
mod scanners;
mod security;
mod util;

fn main() -> anyhow::Result<()> {
    cli::run()
}
