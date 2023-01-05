use clap::Parser;
use concordium_rust_sdk::v2;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "http://localhost:20001")]
    node: v2::Endpoint,
}

fn main() {
    let args = Args::parse();
    println!("{}", args.node.uri());
}
