use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Tabs},
};
use strum::{Display, EnumCount, EnumIter, FromRepr, IntoEnumIterator};
use tokio::sync::mpsc::UnboundedSender;

use super::{
    Component,
    exit_tab::ExitTab,
    prep_tab::PrepTab,
    run_tab::RunTab,
};
use crate::tui::{action::Action, config::Config};

#[derive(Default, Clone, Copy, PartialEq, Eq, Display, EnumIter, EnumCount, FromRepr)]
pub enum Tab {
    #[default]
    #[strum(to_string = "F1 Prep")]
    Prep,
    #[strum(to_string = "F2 Run")]
    Run,
    #[strum(to_string = "F3 Exit")]
    Exit,
}

pub struct TabView {
    selected: Tab,
    prep: PrepTab,
    run: RunTab,
    exit: ExitTab,
    command_tx: Option<UnboundedSender<Action>>,
}

impl TabView {
    pub fn new(config_path: Option<String>) -> Self {
        let mut prep = PrepTab::default();
        prep.set_config_path(config_path);
        Self {
            selected: Tab::default(),
            prep,
            run: RunTab::default(),
            exit: ExitTab::default(),
            command_tx: None,
        }
    }
}

impl Component for TabView {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> color_eyre::Result<()> {
        self.command_tx = Some(tx.clone());
        self.prep.register_action_handler(tx.clone())?;
        self.run.register_action_handler(tx.clone())?;
        self.exit.register_action_handler(tx)?;
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> color_eyre::Result<()> {
        self.prep.register_config_handler(config.clone())?;
        self.run.register_config_handler(config.clone())?;
        self.exit.register_config_handler(config)?;
        Ok(())
    }

    fn init(&mut self, area: ratatui::layout::Size) -> color_eyre::Result<()> {
        self.prep.init(area)?;
        self.run.init(area)?;
        self.exit.init(area)?;
        Ok(())
    }

    fn handle_key_event(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> color_eyre::Result<Option<Action>> {
        use crossterm::event::KeyCode;
        // F-key tab switching — handled here, not via keybinding config
        match key.code {
            KeyCode::F(1) => return Ok(Some(Action::SelectTab(0))),
            KeyCode::F(2) => return Ok(Some(Action::SelectTab(1))),
            KeyCode::F(3) => return Ok(Some(Action::SelectTab(2))),
            KeyCode::Tab => return Ok(Some(Action::TabNext)),
            KeyCode::BackTab => return Ok(Some(Action::TabPrev)),
            _ => {}
        }
        // Delegate remaining keys to the active sub-tab
        match self.selected {
            Tab::Prep => self.prep.handle_key_event(key),
            Tab::Run => self.run.handle_key_event(key),
            Tab::Exit => self.exit.handle_key_event(key),
        }
    }

    fn update(&mut self, action: Action) -> color_eyre::Result<Option<Action>> {
        // Tab switching
        match &action {
            Action::SelectTab(n) => {
                self.selected = Tab::from_repr(*n).unwrap_or_default();
            }
            Action::TabNext => {
                let next = (self.selected as usize + 1) % Tab::COUNT;
                self.selected = Tab::from_repr(next).unwrap_or_default();
            }
            Action::TabPrev => {
                let prev = (self.selected as usize + Tab::COUNT - 1) % Tab::COUNT;
                self.selected = Tab::from_repr(prev).unwrap_or_default();
            }
            _ => {}
        }

        // Forward to all sub-tabs; collect any returned action from the first one
        let mut result = None;
        if let Some(a) = self.prep.update(action.clone())? {
            result = Some(a);
        }
        if let Some(a) = self.run.update(action.clone())? {
            result = result.or(Some(a));
        }
        if let Some(a) = self.exit.update(action)? {
            result = result.or(Some(a));
        }
        Ok(result)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> color_eyre::Result<()> {
        let [tabs_area, content_area, help_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);

        // Render the tab bar
        let titles: Vec<String> = Tab::iter().map(|t| t.to_string()).collect();
        let tabs = Tabs::new(titles)
            .select(self.selected as usize)
            .highlight_style(
                Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .divider("|");
        frame.render_widget(tabs, tabs_area);

        // Wrap content in active-tab border with ► indicator
        let tab_title = match self.selected {
            Tab::Prep => " ► Prep ",
            Tab::Run => " ► Run ",
            Tab::Exit => " ► Exit ",
        };
        let content_block = Block::bordered()
            .title(tab_title)
            .border_style(Style::default().fg(Color::Yellow));
        let inner = content_block.inner(content_area);
        frame.render_widget(content_block, content_area);

        // Render the active sub-tab inside the bordered region
        match self.selected {
            Tab::Prep => self.prep.draw(frame, inner)?,
            Tab::Run => self.run.draw(frame, inner)?,
            Tab::Exit => self.exit.draw(frame, inner)?,
        }

        // Help footer
        let help = Paragraph::new(help_line(self.selected))
            .style(Style::default().fg(Color::DarkGray).bg(Color::Black));
        frame.render_widget(help, help_area);

        Ok(())
    }
}

fn help_line(selected: Tab) -> Line<'static> {
    let key = |s: &'static str| {
        Span::styled(s, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    };
    let desc = |s: &'static str| Span::styled(s, Style::default().fg(Color::DarkGray));

    let mut spans = vec![
        key("[F1]"), desc(" Prep  "),
        key("[F2]"), desc(" Run  "),
        key("[F3]"), desc(" Exit  "),
        key("[Tab]"), desc(" Next  "),
        key("[q]"), desc(" Quit"),
    ];

    match selected {
        Tab::Prep => {
            spans.push(desc("    "));
            spans.push(key("[r]"));
            spans.push(desc(" Run sim  "));
            spans.push(key("[l]"));
            spans.push(desc(" Load config"));
        }
        Tab::Run => {
            spans.push(desc("    "));
            spans.push(key("[p/Space]"));
            spans.push(desc(" Pause  "));
            spans.push(key("[s]"));
            spans.push(desc(" Stop"));
        }
        Tab::Exit => {}
    }

    Line::from(spans)
}
