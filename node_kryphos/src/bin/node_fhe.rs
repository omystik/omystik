use clap::Parser;

fn main() -> anyhow::Result<()> {

    let args = node_kryphos::Args::parse();
    node_kryphos::run(args)
}
