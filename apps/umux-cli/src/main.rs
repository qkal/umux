// SPDX-License-Identifier: GPL-3.0-or-later

use clap::Parser as _;
use umux_cli::Cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.dry_run_json {
        let request = cli.into_request(1)?;
        print!("{}", request.to_json_line()?);
        return Ok(());
    }

    eprintln!("IPC transport is not wired in this foundation slice. Use --dry-run-json.");
    std::process::exit(2);
}
