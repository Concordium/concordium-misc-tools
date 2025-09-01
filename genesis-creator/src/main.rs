/// A command line tool for generating genesis files.
///
/// The tool has two modes: `generate` that can generate a new genesis,
/// potentially reusing some files/keys from the previously generated genesis,
/// and `assemble` that can produce a genesis from existing files (for example
/// to regenereate the Mainnet `genesis.dat`).
///
/// In both modes the tool takes a TOML configuration file that specifies the
/// genesis. For details, see the README.
use clap::Parser;
use genesis_creator::{handle_assemble, handle_generate};
use std::path::PathBuf;

/// Subcommands supported by the tool.
#[derive(clap::Subcommand, Debug)]
#[clap(author, version, about)]
enum GenesisCreatorCommand {
    Assemble {
        #[clap(long, short)]
        /// The TOML configuration file describing the genesis.
        config: PathBuf,
        #[clap(long, short)]
        /// Whether to output additional data during genesis generation.
        verbose: bool,
    },
    Generate {
        #[clap(long, short)]
        /// The TOML configuration file describing the genesis.
        config: PathBuf,
        #[clap(long, short)]
        /// Whether to output additional data during genesis generation.
        verbose: bool,
    },
}

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct GenesisCreator {
    #[clap(subcommand)]
    action: GenesisCreatorCommand,
}

fn main() -> anyhow::Result<()> {
    let args = GenesisCreator::parse();

    match &args.action {
        GenesisCreatorCommand::Assemble { config, verbose } => handle_assemble(config, *verbose),
        GenesisCreatorCommand::Generate { config, verbose } => handle_generate(config, *verbose),
    }
}

// TODO: Deny unused fields.
// TODO: Output genesis_hash
