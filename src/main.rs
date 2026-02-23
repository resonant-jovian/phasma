mod config;
mod app;

use ratatui::Frame;
use ratatui::widgets::Paragraph;

fn main() -> std::io::Result<()> {
    let mut terminal = ratatui::init();
    let result = run_app(&mut terminal);
    ratatui::restore();
    result
}

fn run_app(terminal: &mut ratatui::DefaultTerminal) -> std::io::Result<()> {
    loop {
        terminal.draw(render)?;
        if should_quit()? {
            break Ok(());
        }
    }
}
fn render(frame: &mut ratatui::Frame) {
    let text = Paragraph::new("Hello World!");
    frame.render_widget(text, frame.area());}

fn should_quit() -> std::io::Result<bool> {
   todo!()
}