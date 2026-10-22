use super::*;

impl ReaderApp {
    pub(super) fn smoke_markup_step(&mut self) {
        let key = |key, modifiers| egui::Event::Key {
            key,
            modifiers,
            physical_key: None,
            pressed: true,
            repeat: false,
        };
        match self.smoke_step {
            0 => {
                self.zoom = Zoom::Scale(1.2);
                self.goto(0);
                self.jump = Some((0, 0.25));
            }
            1 => {
                self.smoke_return_y = self.scroll_y;
                let rect = self.smoke_hits["selection-page-0"];
                self.smoke_click =
                    Some(rect.min + Vec2::new(rect.width() * 0.25, rect.height() * 380.0 / 792.0));
            }
            2 => {
                assert_eq!(self.page, 2, "embedded /Dest link must navigate");
                self.click_test_control("back");
            }
            3 => {
                assert_eq!(self.page, 0);
                assert!(
                    (self.scroll_y - self.smoke_return_y).abs() < 3.0,
                    "Back must restore original scroll offset"
                );
                let rect = self.smoke_hits["selection-page-0"];
                self.smoke_click =
                    Some(rect.min + Vec2::new(rect.width() * 0.25, rect.height() * 466.0 / 792.0));
            }
            4 => {
                assert_eq!(self.page, 1, "named GoTo action must navigate");
                self.smoke_events = vec![key(egui::Key::ArrowLeft, egui::Modifiers::ALT)];
            }
            5 => {
                assert_eq!(self.page, 0);
                assert!((self.scroll_y - self.smoke_return_y).abs() < 3.0);
                self.smoke_events = vec![key(egui::Key::ArrowRight, egui::Modifiers::ALT)];
            }
            6 => {
                assert_eq!(self.page, 1);
                self.click_test_control("back");
            }
            7 => {
                assert_eq!(self.page, 0);
                println!(
                    "Embedded directory verified: /Dest and named GoTo clicks, toolbar Back, Alt+Left/Right and original scroll offset"
                );
                self.goto(2);
                self.zoom = Zoom::FitPage;
            }
            8 => {
                let rect = self.smoke_hits["selection-page-2"];
                self.smoke_double_click = Some((
                    rect.min
                        + Vec2::new(rect.width() * 150.0 / 612.0, rect.height() * 72.0 / 792.0),
                    42,
                ));
            }
            9 => {
                assert_eq!(self.selection.current.as_ref().unwrap().text.trim(), "page");
                self.click_test_control("underline-selection");
            }
            10 => {
                assert_eq!(self.highlight_history.active().len(), 1);
                assert_eq!(self.highlight_history.active()[0].kind, MarkKind::Underline);
                self.click_test_control("highlight-colors");
            }
            11 => self.click_test_control("highlight-custom-color"),
            12 => {
                let rect = self.smoke_hits["highlight-palette"];
                self.smoke_click =
                    Some(rect.min + Vec2::new(rect.width() / 3.0, rect.height() * 0.06));
            }
            13 => {
                assert!(self.underline_color[1] > self.underline_color[0] + 100);
                assert_eq!(self.highlight_color, [255, 230, 70]);
                self.click_test_control("highlight-color-apply");
            }
            14 => {
                let rect = self.smoke_hits["selection-page-2"];
                self.smoke_drag = Some((
                    rect.min
                        + Vec2::new(rect.width() * 46.0 / 612.0, rect.height() * 126.0 / 792.0),
                    rect.min
                        + Vec2::new(rect.width() * 380.0 / 612.0, rect.height() * 195.0 / 792.0),
                    0,
                ));
            }
            15 => {
                assert_eq!(self.highlight_history.active().len(), 2);
                assert!(self.highlight_history.active()[1].selection.quads.len() >= 2);
                self.smoke_events = vec![key(egui::Key::Z, egui::Modifiers::CTRL)];
            }
            16 => {
                assert_eq!(self.highlight_history.active().len(), 1);
                self.smoke_events = vec![key(egui::Key::Y, egui::Modifiers::CTRL)];
            }
            17 => {
                assert_eq!(self.highlight_history.active().len(), 2);
                self.smoke_events = vec![key(egui::Key::Escape, egui::Modifiers::NONE)];
            }
            18 => {
                assert!(!self.highlight_tool);
                let saved = lopdf::Document::load(self.path.as_ref().unwrap()).unwrap();
                let annots = saved
                    .get_dictionary(saved.get_pages()[&3])
                    .unwrap()
                    .get(b"Annots")
                    .unwrap()
                    .as_array()
                    .unwrap();
                assert_eq!(annots.len(), 2);
                let annot = saved
                    .get_dictionary(annots[1].as_reference().unwrap())
                    .unwrap();
                assert_eq!(
                    annot.get(b"Subtype").unwrap().as_name().unwrap(),
                    b"Underline"
                );
                for (value, expected) in annot
                    .get(b"C")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .zip(self.underline_color)
                {
                    assert!(
                        (value.as_float().unwrap() - f32::from(expected) / 255.0).abs() < 0.001
                    );
                }
                self.open(self.path.clone().unwrap());
                self.goto(2);
                self.zoom = Zoom::FitPage;
            }
            19 => {
                assert!(self.highlight_history.active().is_empty());
                self.goto(2);
            }
            20 => self.click_test_control("highlight-colors"),
            21 => self.click_test_control("highlight-custom-color"),
            22 => {
                assert!(self.highlight_palette_open);
                println!(
                    "Underline verified: select-first, continuous multiline drag, independent RGB palette, Ctrl+Z/Y, saved standard Underline subtype/color and reopen"
                );
            }
            _ => unreachable!(),
        }
    }
}
