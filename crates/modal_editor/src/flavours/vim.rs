use crate::{visual::visual_block_motion, ModalEditorData};
use editor::{movement, Bias, ToPoint};
use language::Point;
use serde::{Deserialize, Serialize};

use super::*;

#[derive(Default)]
pub struct VimFlavour;
impl ModalEditorFlavour for VimFlavour {
    fn switch_mode(
        &self,
        data: &mut ModalEditorData,
        mode: Mode,
        leave_selections: bool,
        cx: &mut WindowContext,
    ) {
        let state = data.state();
        let last_mode = state.mode;
        let prior_mode = state.last_mode;
        let prior_tx = state.current_tx;
        data.update_state(|state| {
            state.last_mode = last_mode;
            state.mode = mode;
            state.operator_stack.clear();
            state.current_tx.take();
            state.current_anchor.take();
        });
        if mode != Mode::Insert {
            data.take_count(cx);
        }

        // Sync editor settings like clip mode
        data.sync_modal_editor_settings(cx);

        if leave_selections {
            return;
        }

        // Adjust selections
        data.update_active_editor(cx, |_, editor, cx| {
            if last_mode != Mode::VisualBlock && last_mode.is_visual() && mode == Mode::VisualBlock
            {
                visual_block_motion(true, editor, cx, |_, point, goal| Some((point, goal)))
            }
            if last_mode == Mode::Insert || last_mode == Mode::Replace {
                if let Some(prior_tx) = prior_tx {
                    editor.group_until_transaction(prior_tx, cx)
                }
            }

            editor.change_selections(None, cx, |s| {
                // we cheat with visual block mode and use multiple cursors.
                // the cost of this cheat is we need to convert back to a single
                // cursor whenever vim would.
                if last_mode == Mode::VisualBlock
                    && (mode != Mode::VisualBlock && mode != Mode::Insert)
                {
                    let tail = s.oldest_anchor().tail();
                    let head = s.newest_anchor().head();
                    s.select_anchor_ranges(vec![tail..head]);
                } else if last_mode == Mode::Insert
                    && prior_mode == Mode::VisualBlock
                    && mode != Mode::VisualBlock
                {
                    let pos = s.first_anchor().head();
                    s.select_anchor_ranges(vec![pos..pos])
                }

                let snapshot = s.display_map();
                if let Some(pending) = s.pending.as_mut() {
                    if pending.selection.reversed && mode.is_visual() && !last_mode.is_visual() {
                        let mut end = pending.selection.end.to_point(&snapshot.buffer_snapshot);
                        end = snapshot
                            .buffer_snapshot
                            .clip_point(end + Point::new(0, 1), Bias::Right);
                        pending.selection.end = snapshot.buffer_snapshot.anchor_before(end);
                    }
                }

                s.move_with(|map, selection| {
                    if last_mode.is_visual() && !mode.is_visual() {
                        let mut point = selection.head();
                        if !selection.reversed && !selection.is_empty() {
                            point = movement::left(map, selection.head());
                        }
                        selection.collapse_to(point, selection.goal)
                    } else if !last_mode.is_visual() && mode.is_visual() {
                        if selection.is_empty() {
                            selection.end = movement::right(map, selection.start);
                        }
                    } else if last_mode == Mode::Replace {
                        if selection.head().column() != 0 {
                            let point = movement::left(map, selection.head());
                            selection.collapse_to(point, selection.goal)
                        }
                    }
                });
            })
        });
    }

    fn normal_mode(&self) -> Mode {
        Mode::Normal
    }
}
