use gpui::{div, Element, Render, Subscription, ViewContext};
use workspace::{item::ItemHandle, ui::prelude::*, StatusItemView};

use crate::{state::Mode, ModalEditor};

/// The ModeIndicator displays the current mode in the status bar.
pub struct ModeIndicator {
    pub(crate) mode: Option<Mode>,
    pub(crate) operators: String,
    _subscription: Subscription,
}

impl ModeIndicator {
    /// Construct a new mode indicator in this window.
    pub fn new(cx: &mut ViewContext<Self>) -> Self {
        let _subscription = cx.observe_global::<ModalEditor>(|this, cx| this.update_mode(cx));
        let mut this = Self {
            mode: None,
            operators: "".to_string(),
            _subscription,
        };
        this.update_mode(cx);
        this
    }

    fn update_mode(&mut self, cx: &mut ViewContext<Self>) {
        // Modal editor doesn't exist in some tests
        let Some(modal_editor) = cx.try_global::<ModalEditor>() else {
            return;
        };

        if modal_editor.data.enabled {
            self.mode = Some(modal_editor.data.state().mode);
            self.operators = self.current_operators_description(&modal_editor);
        } else {
            self.mode = None;
        }
    }

    fn current_operators_description(&self, modal_editor: &ModalEditor) -> String {
        modal_editor
            .data
            .state()
            .operator_stack
            .iter()
            .map(|item| item.id())
            .collect::<Vec<_>>()
            .join("")
    }
}

impl Render for ModeIndicator {
    fn render(&mut self, _: &mut ViewContext<Self>) -> impl IntoElement {
        let Some(mode) = self.mode.as_ref() else {
            return div().into_any();
        };

        Label::new(format!("{} -- {} --", self.operators, mode))
            .size(LabelSize::Small)
            .line_height_style(LineHeightStyle::UiLabel)
            .into_any_element()
    }
}

impl StatusItemView for ModeIndicator {
    fn set_active_pane_item(
        &mut self,
        _active_pane_item: Option<&dyn ItemHandle>,
        _cx: &mut ViewContext<Self>,
    ) {
        // nothing to do.
    }
}
