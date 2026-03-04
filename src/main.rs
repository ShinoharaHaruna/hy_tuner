mod config;
mod tuner;
mod ui;

use anyhow::Result;

fn main() -> Result<()> {
    ui::run_tui()
}
