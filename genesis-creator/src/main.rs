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

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct GenesisCreator {
    #[clap(subcommand)]
    action: GenesisCreatorCommand,
}

/// Subcommands supported by the genesis-creator tool.
#[derive(clap::Subcommand, Debug)]
#[clap(author, version, about)]
enum GenesisCreatorCommand {
    Assemble {
        #[clap(long, short)]
        /// The TOML configuration file describing the genesis.
        config: std::path::PathBuf,
        #[clap(long, short)]
        /// Whether to output additional data during genesis generation.
        verbose: bool,
    },
    Generate {
        #[clap(long, short)]
        /// The TOML configuration file describing the genesis.
        config: std::path::PathBuf,
        #[clap(long, short)]
        /// Whether to output additional data during genesis generation.
        verbose: bool,
    },
}

fn main() -> anyhow::Result<()> {
    // Initialise tracing with a message-only format so the output matches the
    // previous plain println! style that operators are used to.
    use tracing_subscriber::fmt;
    fmt()
        .with_max_level(tracing::Level::INFO)
        .without_time()
        .with_level(false)
        .with_target(false)
        .init();

    match GenesisCreator::parse().action {
        GenesisCreatorCommand::Assemble { config, verbose } => {
            genesis_creator::handle_assemble(&config, verbose)
        }
        GenesisCreatorCommand::Generate { config, verbose } => {
            genesis_creator::handle_generate(&config, verbose)
        }
    }
}
