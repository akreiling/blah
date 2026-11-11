//! Selection and confirmation state for the paused adaptation commitment panel.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CommitmentPanel {
    selected: usize,
    confirmed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Enter {
    Empty,
    Confirmed,
    Commit(usize),
}

impl CommitmentPanel {
    pub(crate) fn selected(&self, offers: usize) -> Option<usize> {
        if offers == 0 { None } else { Some(self.selected % offers) }
    }

    pub(crate) fn is_confirmed(&self) -> bool {
        self.confirmed
    }

    pub(crate) fn navigate(&mut self, direction: isize, offers: usize) {
        self.confirmed = false;
        if offers == 0 {
            self.selected = 0;
            return;
        }
        let current = self.selected % offers;
        self.selected = (current as isize + direction).rem_euclid(offers as isize) as usize;
    }

    pub(crate) fn enter(&mut self, offers: usize) -> Enter {
        if offers == 0 {
            self.confirmed = false;
            return Enter::Empty;
        }
        if self.confirmed {
            self.confirmed = false;
            Enter::Commit(self.selected % offers)
        } else {
            self.confirmed = true;
            Enter::Confirmed
        }
    }
}

pub(crate) fn cancel(panel: &mut Option<CommitmentPanel>) -> bool {
    panel.take().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_confirms_then_returns_the_selected_offer_for_commit() {
        let mut panel = CommitmentPanel::default();
        assert_eq!(panel.enter(3), Enter::Confirmed);
        assert!(panel.is_confirmed());
        assert_eq!(panel.enter(3), Enter::Commit(0));
        assert!(!panel.is_confirmed());
    }

    #[test]
    fn navigating_changes_selection_and_resets_confirmation() {
        let mut panel = CommitmentPanel::default();
        assert_eq!(panel.enter(3), Enter::Confirmed);
        panel.navigate(1, 3);
        assert_eq!(panel.selected(3), Some(1));
        assert!(!panel.is_confirmed());
        assert_eq!(panel.enter(3), Enter::Confirmed);
        assert_eq!(panel.enter(3), Enter::Commit(1));

        panel.navigate(-1, 3);
        assert_eq!(panel.selected(3), Some(0), "navigation wraps around");
    }

    #[test]
    fn escape_cancels_the_panel_without_a_commit() {
        let mut panel = Some(CommitmentPanel::default());
        assert!(cancel(&mut panel));
        assert_eq!(panel, None);
        assert!(!cancel(&mut panel));
    }
}
