use editor::{movement, scroll::Autoscroll};
use gpui::actions;
use ui::{ViewContext, WindowContext};
use workspace::Workspace;

use crate::{motion::Motion, Vim};

actions!(helix, [ExtendLineBelow,]);

pub fn register(workspace: &mut Workspace, _: &mut ViewContext<Workspace>) {
    workspace.register_action(|_, _: &ExtendLineBelow, cx: &mut ViewContext<Workspace>| {
        extend_line_below()
    })
}

pub fn helix_normal_motion(motion: Motion, times: Option<usize>, cx: &mut WindowContext) {
    Vim::update(cx, |vim, cx| {
        vim.update_active_editor(cx, |_vim, editor, cx| {
            let text_layout_details = editor.text_layout_details(cx);
            editor.change_selections(Some(Autoscroll::fit()), cx, |s| {
                s.move_with(|map, selection| {
                    // let was_reversed = selection.reversed;
                    let current_head = selection.head();

                    selection.start = current_head;
                    selection.end = current_head;

                    // our motions assume the current character is after the cursor,
                    // but in (forward) visual mode the current character is just
                    // before the end of the selection.

                    // If the file ends with a newline (which is common) we don't do this.
                    // so that if you go to the end of such a file you can use "up" to go
                    // to the previous line and have it work somewhat as expected.
                    // #[allow(clippy::nonminimal_bool)]
                    // if !selection.reversed
                    //     && !selection.is_empty()
                    //     && !(selection.end.column() == 0 && selection.end == map.max_point())
                    // {
                    //     current_head = movement::left(map, selection.end)
                    // }

                    let Some((new_head, goal)) = motion.move_point(
                        map,
                        current_head,
                        selection.goal,
                        times,
                        &text_layout_details,
                    ) else {
                        return;
                    };

                    selection.set_head(new_head, goal);

                    // ensure the current character is included in the selection.
                    if (!selection.reversed && !matches!(motion, Motion::NextWordStart { .. }))
                        || (selection.reversed)
                            && !matches!(motion, Motion::PreviousWordStart { .. })
                    {
                        let next_point = movement::right(map, selection.end);

                        if !(next_point.column() == 0 && next_point == map.max_point()) {
                            selection.end = next_point;
                        }
                    }

                    // vim always ensures the anchor character stays selected.
                    // if our selection has reversed, we need to move the opposite end
                    // to ensure the anchor is still selected.
                    // if was_reversed && !selection.reversed {
                    //     selection.start = movement::left(map, selection.start);
                    // } else if !was_reversed && selection.reversed {
                    //     selection.end = movement::right(map, selection.end);
                    // }
                })
            });
        });
    });
}
