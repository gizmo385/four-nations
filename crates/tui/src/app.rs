//! The app: the game session plus what the player sees of it, and the one place keys change them.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use fn_ui::Session;

use crate::input::{Action, map_key};
use crate::state::{Prompt, UiState};

/// The game and what the player sees of it. All input goes through [`App::key`].
pub struct App {
    pub session: Session,
    pub ui: UiState,
    /// Set by anything that changes what the frame shows, cleared when the frame is drawn.
    pub dirty: bool,
}

impl App {
    pub fn new(session: Session, screen: (u16, u16)) -> Self {
        let ui = UiState::new(&session, screen);
        Self { session, ui, dirty: true }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.ui.resize(width, height);
        self.dirty = true;
    }

    /// Handles one key. Keys act between ticks: the loop runs ticks only after the pending keys
    /// are handled, so a command a key issues (#98) never lands in the middle of one.
    pub fn key(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        self.dirty = true;
        if let Some(Prompt::Quit) = self.ui.prompt {
            // Only a plain y confirms; anything else (Esc, n, a stray key) stays.
            self.ui.quit = matches!(key.code, KeyCode::Char('y' | 'Y'));
            self.ui.prompt = None;
            return;
        }
        if let Some(action) = map_key(key) {
            self.act(action);
        }
    }

    pub fn act(&mut self, action: Action) {
        match action {
            Action::Move { dx, dy, tiles } => self.ui.move_cursor(dx * tiles, dy * tiles),
            Action::Jump { dx, dy } => self.ui.jump(dx, dy),
            Action::TogglePause => self.session.clock.toggle_pause(),
            Action::Faster => self.session.clock.faster(),
            Action::Slower => self.session.clock.slower(),
            Action::Step => {
                self.session.step();
            }
            Action::AskQuit => self.ui.prompt = Some(Prompt::Quit),
            Action::QuitNow => self.ui.quit = true,
        }
    }

    /// Runs the ticks `real_secs` of wall time is worth.
    pub fn advance(&mut self, real_secs: f64) {
        if self.session.advance(real_secs) > 0 {
            self.dirty = true;
        }
    }
}
