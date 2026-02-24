use clap::Parser;
use tui::cli::Cli;
use tui::app::App;
mod tui;
mod toml;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    tui::errors::init()?;
    tui::logging::init()?;

    let args = Cli::parse();
    let mut app = App::new(args.tick_rate, args.frame_rate)?;
    app.run().await?;
    Ok(())
}
