//! Modal editing support for Zed
#![allow(unused_imports)]

#[cfg(test)]
mod test;

mod command;
mod editor_events;
mod insert;
mod mode_indicator;
mod motion;
mod normal;
mod object;
mod replace;
mod state;
mod surrounds;
mod utils;
mod vim;
mod visual;

use anyhow::Result;
use collections::HashMap;
use command_palette_hooks::{CommandPaletteFilter, CommandPaletteInterceptor};
use editor::{
    movement::{self, FindRange},
    Anchor, Bias, Editor, EditorEvent, EditorMode, ToPoint,
};
use gpui::{
    actions, impl_actions, Action, AppContext, EntityId, FocusableView, Global, KeystrokeEvent,
    Subscription, View, ViewContext, WeakView, WindowContext,
};
use language::{CursorShape, Point, SelectionGoal, TransactionId};
pub use mode_indicator::ModeIndicator;
use motion::Motion;
use normal::normal_replace;
use replace::multi_replace;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_derive::Serialize;
use settings::{update_settings_file, Settings, SettingsSources, SettingsStore};
use state::{EditorState, Mode, Operator, RecordedSelection, WorkspaceState};
use std::{ops::Range, sync::Arc};
use surrounds::{add_surrounds, change_surrounds, delete_surrounds};
use ui::BorrowAppContext;
use visual::{visual_block_motion, visual_replace};
use workspace::{self, Workspace};

use crate::state::ReplayableAction;

trait ModalEditorMethod {}

#[derive(Default)]
struct ModalEditor {
    active_editor: Option<WeakView<Editor>>,
    editor_subscription: Option<Subscription>,
    editor_states: HashMap<EntityId, EditorState>,
    default_state: EditorState,
}

impl Global for ModalEditor {}

impl ModalEditor {
    const NAMESPACE: &'static str = "modal_editor";

    fn read(cx: &mut AppContext) -> &Self {
        cx.global::<Self>()
    }

    fn update<F, S>(cx: &mut WindowContext, update: F) -> S
    where
        F: FnOnce(&mut Self, &mut WindowContext) -> S,
    {
        cx.update_global(update)
    }

    fn activate_editor(&mut self, editor: View<Editor>, cx: &mut WindowContext) {
        if !editor.read(cx).use_modal_editing() {
            return;
        }

        // self.active_editor = Some(editor.clone().downgrade());
        // self.editor_subscription = Some(cx.subscribe(&editor, |editor, event, cx| match event {
        //     EditorEvent::SelectionsChanged { local: true } => {
        //         if editor.read(cx).leader_peer_id().is_none() {
        //             ModalEditor::update(cx, |vim, cx| {
        //                 vim.local_selections_changed(editor, cx);
        //             })
        //         }
        //     }
        //     EditorEvent::InputIgnored { text } => {
        //         ModalEditor::active_editor_input_ignored(text.clone(), cx);
        //         ModalEditor::record_insertion(text, None, cx)
        //     }
        //     EditorEvent::InputHandled {
        //         text,
        //         utf16_range_to_replace: range_to_replace,
        //     } => ModalEditor::record_insertion(text, range_to_replace.clone(), cx),
        //     EditorEvent::TransactionBegun { transaction_id } => {
        //         ModalEditor::update(cx, |vim, cx| {
        //             vim.transaction_begun(*transaction_id, cx);
        //         })
        //     }
        //     EditorEvent::TransactionUndone { transaction_id } => {
        //         ModalEditor::update(cx, |vim, cx| {
        //             vim.transaction_undone(transaction_id, cx);
        //         })
        //     }
        //     _ => {}
        // }));

        // TODO: Vim specific behaviour needing abstraction
        // let editor = editor.read(cx);
        // let editor_mode = editor.mode();
        // let newest_selection_empty = editor.selections.newest::<usize>(cx).is_empty();

        // if editor_mode == EditorMode::Full
        //         && !newest_selection_empty
        //         && self.state().mode == Mode::Normal
        //         // When following someone, don't switch vim mode.
        //         && editor.leader_peer_id().is_none()
        // {
        //     self.switch_mode(Mode::Visual, true, cx);
        // }

        self.sync_vim_settings(cx);
    }

    // fn record_insertion(
    //     text: &Arc<str>,
    //     range_to_replace: Option<Range<isize>>,
    //     cx: &mut WindowContext,
    // ) {
    //     ModalEditor::update(cx, |me, _| {
    //         if me.workspace_state.recording {
    //             me.workspace_state
    //                 .recorded_actions
    //                 .push(ReplayableAction::Insertion {
    //                     text: text.clone(),
    //                     utf16_range_to_replace: range_to_replace,
    //                 });
    //             if me.workspace_state.stop_recording_after_next_action {
    //                 me.workspace_state.recording = false;
    //                 me.workspace_state.stop_recording_after_next_action = false;
    //             }
    //         }
    //     });
    // }

    // fn update_active_editor<S>(
    //     &mut self,
    //     cx: &mut WindowContext,
    //     update: impl FnOnce(&mut ModalEditor, &mut Editor, &mut ViewContext<Editor>) -> S,
    // ) -> Option<S> {
    //     let editor = self.active_editor.clone()?.upgrade()?;
    //     Some(editor.update(cx, |editor, cx| update(self, editor, cx)))
    // }

    // fn editor_selections(&mut self, cx: &mut WindowContext) -> Vec<Range<Anchor>> {
    //     self.update_active_editor(cx, |_, editor, _| {
    //         editor
    //             .selections
    //             .disjoint_anchors()
    //             .iter()
    //             .map(|selection| selection.tail()..selection.head())
    //             .collect()
    //     })
    //     .unwrap_or_default()
    // }

    // /// When doing an action that modifies the buffer, we start recording so that `.`
    // /// will replay the action.
    // pub fn start_recording(&mut self, cx: &mut WindowContext) {
    //     if !self.workspace_state.replaying {
    //         self.workspace_state.recording = true;
    //         self.workspace_state.recorded_actions = Default::default();
    //         self.workspace_state.recorded_count = None;

    //         let selections = self
    //             .active_editor
    //             .as_ref()
    //             .and_then(|editor| editor.upgrade())
    //             .map(|editor| {
    //                 let editor = editor.read(cx);
    //                 (
    //                     editor.selections.oldest::<Point>(cx),
    //                     editor.selections.newest::<Point>(cx),
    //                 )
    //             });

    //         if let Some((oldest, newest)) = selections {
    //             self.workspace_state.recorded_selection = match self.state().mode {
    //                 Mode::Visual if newest.end.row == newest.start.row => {
    //                     RecordedSelection::SingleLine {
    //                         cols: newest.end.column - newest.start.column,
    //                     }
    //                 }
    //                 Mode::Visual => RecordedSelection::Visual {
    //                     rows: newest.end.row - newest.start.row,
    //                     cols: newest.end.column,
    //                 },
    //                 Mode::VisualLine => RecordedSelection::VisualLine {
    //                     rows: newest.end.row - newest.start.row,
    //                 },
    //                 Mode::VisualBlock => RecordedSelection::VisualBlock {
    //                     rows: newest.end.row.abs_diff(oldest.start.row),
    //                     cols: newest.end.column.abs_diff(oldest.start.column),
    //                 },
    //                 _ => RecordedSelection::None,
    //             }
    //         } else {
    //             self.workspace_state.recorded_selection = RecordedSelection::None;
    //         }
    //     }
    // }

    // pub fn stop_replaying(&mut self) {
    //     self.workspace_state.replaying = false;
    // }

    // /// When finishing an action that modifies the buffer, stop recording.
    // /// as you usually call this within a keystroke handler we also ensure that
    // /// the current action is recorded.
    // pub fn stop_recording(&mut self) {
    //     if self.workspace_state.recording {
    //         self.workspace_state.stop_recording_after_next_action = true;
    //     }
    // }

    // /// Stops recording actions immediately rather than waiting until after the
    // /// next action to stop recording.
    // ///
    // /// This doesn't include the current action.
    // pub fn stop_recording_immediately(&mut self, action: Box<dyn Action>) {
    //     if self.workspace_state.recording {
    //         self.workspace_state
    //             .recorded_actions
    //             .push(ReplayableAction::Action(action.boxed_clone()));
    //         self.workspace_state.recording = false;
    //         self.workspace_state.stop_recording_after_next_action = false;
    //     }
    // }

    // /// Explicitly record one action (equivalents to start_recording and stop_recording)
    // pub fn record_current_action(&mut self, cx: &mut WindowContext) {
    //     self.start_recording(cx);
    //     self.stop_recording();
    // }

    /// Returns the state of the active editor.
    pub fn state(&self) -> &EditorState {
        if let Some(active_editor) = self.active_editor.as_ref() {
            if let Some(state) = self.editor_states.get(&active_editor.entity_id()) {
                return state;
            }
        }

        &self.default_state
    }

    fn update_active_editor<S>(
        &mut self,
        cx: &mut WindowContext,
        update: impl FnOnce(&mut ModalEditor, &mut Editor, &mut ViewContext<Editor>) -> S,
    ) -> Option<S> {
        let editor = self.active_editor.clone()?.upgrade()?;
        Some(editor.update(cx, |editor, cx| update(self, editor, cx)))
    }

    fn sync_vim_settings(&mut self, cx: &mut WindowContext) {
        self.update_active_editor(cx, |modal_editor, editor, cx| {
            let state = modal_editor.state();
            editor.set_cursor_shape(state.cursor_shape(), cx);
            editor.set_clip_at_line_ends(state.clip_at_line_ends(), cx);
            editor.set_collapse_matches(true);
            editor.set_input_enabled(!state.vim_controlled());
            editor.set_autoindent(state.should_autoindent());
            editor.selections.line_mode = matches!(state.mode, Mode::VisualLine);
            if editor.is_focused(cx) {
                editor.set_keymap_context_layer::<Self>(state.keymap_context_layer(), cx);
            // disables vim if the rename editor is focused,
            // but not if the command palette is open.
            } else if editor.focus_handle(cx).contains_focused(cx) {
                editor.remove_keymap_context_layer::<Self>(cx)
            }
        });
    }
}

pub fn init(cx: &mut AppContext) {
    cx.set_global(ModalEditor::default());
    ModalEditorSettings::register(cx);
}

/// Which modal editing method to use (work in progress).
///
/// Default: None
#[derive(Copy, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum ModalEditingMethodSetting {
    #[default]
    None,
    Vim,
}

impl Settings for ModalEditingMethodSetting {
    const KEY: Option<&'static str> = Some("modal_editing_method");

    type FileContent = Option<ModalEditingMethodSetting>;

    fn load(sources: SettingsSources<Self::FileContent>, _: &mut AppContext) -> Result<Self> {
        if let Some(Some(user_value)) = sources.user.copied() {
            return Ok(user_value);
        }
        sources.default.ok_or_else(Self::missing_default)
    }
}

/// Controls when to use system clipboard.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UseSystemClipboard {
    /// Don't use system clipboard.
    Never,
    /// Use system clipboard.
    Always,
    /// Use system clipboard for yank operations.
    OnYank,
}

#[derive(Deserialize)]
struct ModalEditorSettings {
    // all vim uses vim clipboard
    // vim always uses system cliupbaord
    // some magic where yy is system and dd is not.
    pub use_system_clipboard: UseSystemClipboard,
    pub use_multiline_find: bool,
    pub use_smartcase_find: bool,
}

#[derive(Clone, Default, Serialize, Deserialize, JsonSchema)]
struct ModalEditorSettingsContent {
    pub use_system_clipboard: Option<UseSystemClipboard>,
    pub use_multiline_find: Option<bool>,
    pub use_smartcase_find: Option<bool>,
}

impl Settings for ModalEditorSettings {
    const KEY: Option<&'static str> = Some("modal_editor");

    type FileContent = ModalEditorSettingsContent;

    fn load(sources: SettingsSources<Self::FileContent>, _: &mut AppContext) -> Result<Self> {
        sources.json_merge()
    }
}
