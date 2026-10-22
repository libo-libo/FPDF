use eframe::egui;

/// Reader commands must not steal a focused editor's keys before it is drawn.
/// Dialog flags still belong in the caller: a dialog can open this very frame.
pub fn reader_shortcuts_allowed(ctx: &egui::Context) -> bool {
    !ctx.wants_keyboard_input()
        && !ctx.is_popup_open()
        && ctx.memory(|memory| memory.top_modal_layer().is_none())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::Range;

    struct Editor {
        ctx: egui::Context,
        text: String,
        selection: Range<usize>,
        time: f64,
        dialog: bool,
    }

    impl Editor {
        fn new(multiline: bool, dialog: bool) -> Self {
            let mut editor = Self {
                ctx: egui::Context::default(),
                text: "alpha beta\ngamma".into(),
                selection: 0..0,
                time: 0.0,
                dialog,
            };
            editor.frame(vec![], multiline, false);
            editor.frame(vec![], multiline, true);
            editor
        }

        fn frame(
            &mut self,
            events: Vec<egui::Event>,
            multiline: bool,
            expect_editor: bool,
        ) -> egui::FullOutput {
            self.time += 1.0;
            self.ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    )),
                    time: Some(self.time),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    if expect_editor {
                        // This deliberately runs before the TextEdit, as app shortcuts do.
                        assert!(!reader_shortcuts_allowed(ctx));
                    }
                    if reader_shortcuts_allowed(ctx) {
                        ctx.input_mut(|input| {
                            for key in [
                                egui::Key::A,
                                egui::Key::Z,
                                egui::Key::Y,
                                egui::Key::Home,
                                egui::Key::End,
                            ] {
                                input.consume_key(egui::Modifiers::COMMAND, key);
                            }
                        });
                    }
                    let draw_editor = |ui: &mut egui::Ui| {
                        let edit = if multiline {
                            egui::TextEdit::multiline(&mut self.text)
                        } else {
                            egui::TextEdit::singleline(&mut self.text)
                        };
                        let output = edit.id(egui::Id::new("editor")).show(ui);
                        output.response.request_focus();
                        if let Some(cursor_range) = output.cursor_range {
                            self.selection = cursor_range.as_sorted_char_range();
                        }
                    };
                    if self.dialog {
                        egui::Window::new("Editor dialog").show(ctx, draw_editor);
                    } else {
                        egui::CentralPanel::default().show(ctx, draw_editor);
                    }
                },
            )
        }

        fn send(&mut self, events: Vec<egui::Event>, multiline: bool) -> egui::FullOutput {
            self.frame(events, multiline, true)
        }
    }

    fn ctrl(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            },
        }
    }

    fn copied(output: egui::FullOutput) -> Option<String> {
        output
            .platform_output
            .commands
            .into_iter()
            .find_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text),
                _ => None,
            })
    }

    #[test]
    fn focused_editors_keep_select_copy_cut_paste_undo_and_redo() {
        for (multiline, dialog) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut editor = Editor::new(multiline, dialog);
            let original = editor.text.clone();
            editor.send(vec![ctrl(egui::Key::A)], multiline);
            assert_eq!(editor.selection, 0..original.chars().count());
            assert_eq!(
                copied(editor.send(vec![egui::Event::Copy], multiline)),
                Some(original.clone())
            );
            assert_eq!(
                copied(editor.send(vec![egui::Event::Cut], multiline)),
                Some(original.clone())
            );
            assert!(editor.text.is_empty());
            editor.send(vec![ctrl(egui::Key::Z)], multiline);
            assert_eq!(editor.text, original);
            editor.send(vec![ctrl(egui::Key::Y)], multiline);
            assert!(editor.text.is_empty());
            editor.send(vec![egui::Event::Paste("replacement".into())], multiline);
            assert_eq!(editor.text, "replacement");
            editor.send(vec![ctrl(egui::Key::Z)], multiline);
            assert!(editor.text.is_empty());
            editor.send(vec![ctrl(egui::Key::Y)], multiline);
            assert_eq!(editor.text, "replacement");
            editor.send(vec![ctrl(egui::Key::Z)], multiline);
            assert!(editor.text.is_empty());
            let mut redo = ctrl(egui::Key::Z);
            if let egui::Event::Key { modifiers, .. } = &mut redo {
                modifiers.shift = true;
            }
            editor.send(vec![redo], multiline);
            assert_eq!(editor.text, "replacement");
        }
    }

    #[test]
    fn ctrl_home_and_end_move_the_editor_cursor_instead_of_the_document() {
        for (multiline, dialog) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut editor = Editor::new(multiline, dialog);
            editor.send(vec![ctrl(egui::Key::Home)], multiline);
            assert_eq!(editor.selection, 0..0);
            editor.send(vec![ctrl(egui::Key::End)], multiline);
            let end = editor.text.chars().count();
            assert_eq!(editor.selection, end..end);
        }
    }

    #[test]
    fn document_shortcuts_resume_after_focus_leaves_the_editor() {
        let editor = Editor::new(true, false);
        editor
            .ctx
            .memory_mut(|memory| memory.surrender_focus(egui::Id::new("editor")));
        let _ = editor.ctx.run(
            egui::RawInput {
                events: vec![ctrl(egui::Key::Z)],
                ..Default::default()
            },
            |ctx| {
                assert!(reader_shortcuts_allowed(ctx));
                assert!(ctx.input_mut(|input| {
                    input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)
                }));
            },
        );
    }

    #[test]
    fn modal_without_a_text_field_also_blocks_document_commands() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::Modal::new(egui::Id::new("confirmation")).show(ctx, |ui| {
                ui.label("Confirm action");
            });
        });
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            assert!(!ctx.wants_keyboard_input());
            assert!(!reader_shortcuts_allowed(ctx));
        });
    }
}
