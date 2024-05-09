use crate::{insert::NormalBefore, ModalEditor, ModalEditorFlavourSetting};
use editor::{Editor, EditorEvent};
use gpui::{
    Action, AppContext, BorrowAppContext, Entity, EntityId, View, ViewContext, WindowContext,
};
use settings::{Settings, SettingsStore};

pub fn init(cx: &mut AppContext) {
    cx.observe_new_views(|_, cx: &mut ViewContext<Editor>| {
        let editor = cx.view().clone();
        cx.subscribe(&editor, |_, editor, event: &EditorEvent, cx| match event {
            EditorEvent::Focused => cx.window_context().defer(|cx| focused(editor, cx)),
            EditorEvent::Blurred => cx.window_context().defer(|cx| blurred(editor, cx)),
            _ => {}
        })
        .detach();

        let enabled = *ModalEditorFlavourSetting::get_global(cx) != ModalEditorFlavourSetting::None;
        cx.observe_global::<SettingsStore>(move |editor, cx| {
            let new_enabled =
                *ModalEditorFlavourSetting::get_global(cx) != ModalEditorFlavourSetting::None;
            if new_enabled != enabled {
                if !new_enabled {
                    ModalEditor::unhook_vim_settings(editor, cx);
                }
            }
        })
        .detach();

        let id = cx.view().entity_id();
        cx.on_release(move |_, _, cx| released(id, cx)).detach();
    })
    .detach();
}
fn focused(editor: View<Editor>, cx: &mut WindowContext) {
    ModalEditor::update(cx, |modal_editor, cx| {
        if !modal_editor.data.enabled {
            return;
        }
        modal_editor.activate_editor(editor.clone(), cx);
    });
}

fn blurred(editor: View<Editor>, cx: &mut WindowContext) {
    ModalEditor::update(cx, |modal_editor, cx| {
        if !modal_editor.data.enabled {
            return;
        }
        if let Some(previous_editor) = modal_editor.data.active_editor.clone() {
            modal_editor.stop_recording_immediately(NormalBefore.boxed_clone());
            if previous_editor
                .upgrade()
                .is_some_and(|previous| previous == editor.clone())
            {
                modal_editor.clear_operator(cx);
            }
        }
        editor.update(cx, |editor, cx| {
            if editor.use_modal_editing() {
                editor.set_cursor_shape(language::CursorShape::Hollow, cx);
            }
        });
    });
}

fn released(entity_id: EntityId, cx: &mut AppContext) {
    cx.update_global(|modal_editor: &mut ModalEditor, _| {
        if modal_editor
            .data
            .active_editor
            .as_ref()
            .is_some_and(|previous| previous.entity_id() == entity_id)
        {
            modal_editor.data.active_editor = None;
            modal_editor.data.editor_subscription = None;
        }
        modal_editor.data.editor_states.remove(&entity_id)
    });
}

#[cfg(test)]
mod test {
    use crate::{test::VimTestContext, vim::Vim};
    use editor::Editor;
    use gpui::{Context, Entity, VisualTestContext};
    use language::Buffer;

    // regression test for blur called with a different active editor
    #[gpui::test]
    async fn test_blur_focus(cx: &mut gpui::TestAppContext) {
        let mut cx = VimTestContext::new(cx, true).await;

        let buffer = cx.new_model(|cx| Buffer::local("a = 1\nb = 2\n", cx));
        let window2 = cx.add_window(|cx| Editor::for_buffer(buffer, None, cx));
        let editor2 = cx
            .update(|cx| {
                window2.update(cx, |_, cx| {
                    cx.activate_window();
                    cx.focus_self();
                    cx.view().clone()
                })
            })
            .unwrap();
        cx.run_until_parked();

        cx.update(|cx| {
            let vim = Vim::read(cx);
            assert_eq!(
                vim.active_editor.as_ref().unwrap().entity_id(),
                editor2.entity_id(),
            )
        });

        // no panic when blurring an editor in a different window.
        cx.update_editor(|editor1, cx| {
            editor1.handle_blur(cx);
        });
    }

    // regression test for focus_in/focus_out being called on window activation
    #[gpui::test]
    async fn test_focus_across_windows(cx: &mut gpui::TestAppContext) {
        let mut cx = VimTestContext::new(cx, true).await;

        let mut cx1 = VisualTestContext::from_window(cx.window, &cx);
        let editor1 = cx.editor.clone();

        let buffer = cx.new_model(|cx| Buffer::local("a = 1\nb = 2\n", cx));
        let (editor2, cx2) = cx.add_window_view(|cx| Editor::for_buffer(buffer, None, cx));

        editor2.update(cx2, |_, cx| {
            cx.focus_self();
            cx.activate_window();
        });
        cx.run_until_parked();

        cx1.update(|cx| {
            assert_eq!(
                Vim::read(cx).active_editor.as_ref().unwrap().entity_id(),
                editor2.entity_id(),
            )
        });

        cx1.update(|cx| {
            cx.activate_window();
        });
        cx.run_until_parked();

        cx.update(|cx| {
            assert_eq!(
                Vim::read(cx).active_editor.as_ref().unwrap().entity_id(),
                editor1.entity_id(),
            )
        });
    }
}
