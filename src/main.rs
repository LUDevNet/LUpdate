use argh::FromArgs;
use log::LevelFilter;

mod cache;
mod pack;
mod pki;

#[derive(FromArgs, PartialEq, Debug)]
/// CLI to update a patch server
struct Args {
    #[argh(subcommand)]
    nested: Commands,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
pub enum Commands {
    Cache(cache::Args),
    Pack(pack::Args),
    PKI(pki::Args),
}

fn main() -> color_eyre::Result<()> {
    env_logger::builder()
        .format_timestamp(None)
        .filter_level(LevelFilter::Info)
        .init();

    let args: Args = argh::from_env();

    match args.nested {
        Commands::Cache(args) => cache::run(args),
        Commands::Pack(args) => pack::run(args),
        Commands::PKI(args) => pki::run(args),
    }
}
