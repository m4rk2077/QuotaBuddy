#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PanelAction {
    Show,
    Hide,
}

#[derive(Default)]
pub struct TrayClickTracker {
    visible_on_left_down: Option<bool>,
}

impl TrayClickTracker {
    pub fn left_down(&mut self, panel_is_visible: bool) {
        self.visible_on_left_down = Some(panel_is_visible);
    }

    pub fn left_up(&mut self, panel_is_visible: bool) -> PanelAction {
        let was_visible = self.visible_on_left_down.take().unwrap_or(panel_is_visible);
        if was_visible {
            PanelAction::Hide
        } else {
            PanelAction::Show
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PanelAction, TrayClickTracker};

    #[test]
    fn a_focus_loss_between_left_down_and_left_up_cannot_reopen_a_visible_panel() {
        let mut tracker = TrayClickTracker::default();
        tracker.left_down(true);

        assert_eq!(tracker.left_up(false), PanelAction::Hide);
    }

    #[test]
    fn left_click_opens_a_panel_that_was_hidden_when_the_press_started() {
        let mut tracker = TrayClickTracker::default();
        tracker.left_down(false);

        assert_eq!(tracker.left_up(false), PanelAction::Show);
    }

    #[test]
    fn an_unpaired_release_falls_back_to_current_visibility() {
        let mut tracker = TrayClickTracker::default();

        assert_eq!(tracker.left_up(false), PanelAction::Show);
        assert_eq!(tracker.left_up(true), PanelAction::Hide);
    }
}
