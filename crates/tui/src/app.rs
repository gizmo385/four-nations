//! The app: the game session plus what the player sees of it, and the one place keys change them.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use fn_ui::tools::{apply_rect, switch_tool};
use fn_ui::{Session, Tool};

use crate::input::{Action, map_key};
use crate::keys::Step;
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
    /// are handled, so a command a key issues never lands in the middle of one.
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
            Action::PickTool(tool) => self.pick_tool(tool),
            Action::Confirm => self.confirm(),
            Action::Cancel => self.cancel(),
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

    /// A tool key: the tool becomes active, and the corner and the last result are dropped
    /// (pressing the active tool's own key again does just that).
    fn pick_tool(&mut self, requested: Tool) {
        // The shared switch also resolves the Build tool's building; the keys here never ask for
        // Build, so there is no choice to keep.
        let (tool, _choice) = switch_tool(&self.session, self.ui.tool, None, requested);
        self.ui.set_tool(tool);
    }

    /// `Enter` in a tool: the first sets a corner, the second applies the tool to every tile of
    /// the rectangle through `fn_ui`'s shared logic. The tool stays active for the next one. In
    /// Select mode `Enter` is for selecting a unit (#99).
    fn confirm(&mut self) {
        if self.ui.tool == Tool::Select {
            return;
        }
        match self.ui.anchor.take() {
            None => self.ui.anchor = Some(self.ui.cursor),
            Some(corner) => {
                // The shared logic also takes the Shift state (a repeating harvest) and the
                // Build tool's building; neither has a key here yet.
                self.ui.notice = apply_rect(
                    &mut self.session,
                    self.ui.tool,
                    corner,
                    self.ui.cursor,
                    false,
                    None,
                );
            }
        }
    }

    /// `Esc` in a tool: drop the corner, or with none leave the tool.
    fn cancel(&mut self) {
        if self.ui.anchor.take().is_none() {
            self.ui.set_tool(Tool::Select);
        }
    }

    /// Runs the ticks `real_secs` of wall time is worth.
    pub fn advance(&mut self, real_secs: f64) {
        if self.session.advance(real_secs) > 0 {
            self.dirty = true;
        }
    }

    /// Plays a key script (`--keys`): each key as if pressed between ticks, and `<tick N>` as N
    /// ticks of the world whatever the clock says. No time passes except through those, so a
    /// seed and a script always give the same game.
    pub fn play(&mut self, steps: &[Step]) {
        for step in steps {
            match *step {
                Step::Key(key) => self.key(key),
                Step::Tick(n) => {
                    self.session.tick_n(n);
                    self.dirty = true;
                }
            }
            if self.ui.quit {
                return;
            }
        }
    }
}
