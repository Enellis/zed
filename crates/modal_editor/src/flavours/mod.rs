pub mod vim;

use std::fmt::Display;

use crate::{state::Mode, ModalEditor, ModalEditorData};
use serde::{Deserialize, Serialize};
use ui::WindowContext;

use self::vim::VimFlavour;

pub trait ModalEditorFlavour {
    fn switch_mode(
        &self,
        data: &mut ModalEditorData,
        mode: Mode,
        leave_selections: bool,
        cx: &mut WindowContext,
    );

    fn normal_mode(&self) -> Mode;
}

impl Default for Box<dyn ModalEditorFlavour> {
    fn default() -> Self {
        Box::new(VimFlavour {})
    }
}
