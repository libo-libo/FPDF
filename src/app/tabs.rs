use super::*;
use eframe::App;

pub(super) enum TabAction {
    New,
    Close,
    Switch(usize),
}

struct Tab {
    id: u64,
    app: ReaderApp,
    memory: Option<egui::Memory>,
}

pub struct WindowApp {
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
    blank_memory: egui::Memory,
    controls: HashMap<String, Rect>,
    smoke_tabs: bool,
    smoke_step: u8,
    smoke_frames: u8,
    started: Instant,
}

impl WindowApp {
    pub fn new(
        ctx: &egui::Context,
        input: Option<PathBuf>,
        screenshot: Option<PathBuf>,
        smoke_features: bool,
        smoke_outline: bool,
        smoke_reader: bool,
        extra: ExtraSmoke,
    ) -> Self {
        let smoke_tabs = matches!(extra, ExtraSmoke::Tabs);
        let app = ReaderApp::new(
            ctx,
            input,
            screenshot,
            smoke_features,
            smoke_outline,
            smoke_reader,
            extra,
        );
        Self {
            tabs: vec![Tab {
                id: 1,
                app,
                memory: None,
            }],
            active: 0,
            next_id: 2,
            blank_memory: ctx.memory(Clone::clone),
            controls: HashMap::new(),
            smoke_tabs,
            smoke_step: 0,
            smoke_frames: 0,
            started: Instant::now(),
        }
    }

    fn switch(&mut self, ctx: &egui::Context, index: usize) {
        if index == self.active || index >= self.tabs.len() {
            return;
        }
        let previous = &mut self.tabs[self.active];
        previous.memory = Some(ctx.memory(Clone::clone));
        // Keep the current page warm without retaining a full render cache per inactive tab.
        let page = previous.app.page;
        previous
            .app
            .cache
            .retain(|entry| entry.target.page == page && !entry.target.thumbnail);
        previous.app.cache.truncate(1);
        previous.app.thumbnails.clear();
        self.active = index;
        let next = &mut self.tabs[index];
        ctx.memory_mut(|m| {
            *m = next
                .memory
                .take()
                .unwrap_or_else(|| self.blank_memory.clone())
        });
        next.app.applied_theme = None;
        next.app.last_plan.clear();
        next.app.started = Instant::now();
        ctx.request_repaint();
    }

    fn add(&mut self, ctx: &egui::Context, path: Option<PathBuf>) {
        if let Some(path) = &path {
            let identity = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
            if let Some(index) = self.tabs.iter().position(|tab| {
                tab.app.path.as_ref().is_some_and(|p| {
                    std::fs::canonicalize(p)
                        .unwrap_or_else(|_| p.clone())
                        .to_string_lossy()
                        .to_lowercase()
                        == identity.to_string_lossy().to_lowercase()
                })
            }) {
                self.switch(ctx, index);
                return;
            }
            if self.tabs[self.active].app.path.is_none() {
                self.tabs[self.active].app.open(path.clone());
                return;
            }
        }
        let mut app = ReaderApp::new(
            ctx,
            path,
            None,
            self.smoke_tabs,
            false,
            false,
            ExtraSmoke::None,
        );
        app.smoke_tabs = self.smoke_tabs;
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(Tab {
            id,
            app,
            memory: None,
        });
        self.switch(ctx, self.tabs.len() - 1);
    }

    fn can_close(tab: &mut Tab) -> bool {
        if tab.app.highlight_pending.is_some() {
            tab.app.selection.notice = "正在保存高亮，请稍候再关闭标签页。".into();
            return false;
        }
        if tab
            .app
            .outline_editor
            .as_mut()
            .is_some_and(|e| e.guard_close())
            || tab
                .app
                .inline_rename
                .as_ref()
                .is_some_and(|r| r.changed() || r.result.is_some())
        {
            tab.app.outline_notice = Some("目录有未保存的修改，请先保存或取消。".into());
            return false;
        }
        true
    }

    fn close(&mut self, ctx: &egui::Context, index: usize) {
        if !Self::can_close(&mut self.tabs[index]) {
            self.switch(ctx, index);
            return;
        }
        if self.tabs.len() == 1 {
            self.add(ctx, None);
        }
        if self.active == index {
            self.switch(ctx, if index > 0 { index - 1 } else { 1 });
        }
        self.tabs.remove(index);
        if index < self.active {
            self.active -= 1;
        }
        ctx.request_repaint();
    }

    fn tab_bar(&mut self, ctx: &egui::Context) -> Option<(TabAction, Option<usize>)> {
        let mut action = None;
        self.controls.clear();
        egui::TopBottomPanel::top("document-tabs")
            .exact_height(36.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    egui::ScrollArea::horizontal()
                        .id_salt("document-tab-scroll")
                        .max_width((ui.available_width() - 48.0).max(100.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for (index, tab) in self.tabs.iter().enumerate() {
                                    ui.push_id(tab.id, |ui| {
                                        let path = tab.app.path.as_ref();
                                        let name = path
                                            .and_then(|p| p.file_name())
                                            .map(|p| p.to_string_lossy().into_owned())
                                            .unwrap_or_else(|| "新标签页".into());
                                        let label = if tab.app.highlight_pending.is_some() {
                                            format!("{name} · 保存中")
                                        } else {
                                            name
                                        };
                                        let button = ui
                                            .add_sized(
                                                [190.0, 28.0],
                                                egui::Button::new(egui::RichText::new(label))
                                                    .selected(index == self.active)
                                                    .truncate(),
                                            )
                                            .on_hover_text(
                                                path.map(|p| p.display().to_string())
                                                    .unwrap_or_else(|| "打开 PDF 开始阅读".into()),
                                            );
                                        self.controls
                                            .insert(format!("tab-{}", tab.id), button.rect);
                                        if button.clicked() {
                                            action = Some((TabAction::Switch(index), None));
                                        }
                                        let close = ui
                                            .small_button("×")
                                            .on_hover_text("关闭标签页 · Ctrl+W");
                                        self.controls
                                            .insert(format!("close-{}", tab.id), close.rect);
                                        if close.clicked()
                                            || button.clicked_by(egui::PointerButton::Middle)
                                        {
                                            action = Some((TabAction::Close, Some(index)));
                                        }
                                    });
                                }
                            });
                        });
                    let plus = ui.button("＋").on_hover_text("新标签页 · Ctrl+T");
                    self.controls.insert("new-tab".into(), plus.rect);
                    if plus.clicked() {
                        action = Some((TabAction::New, None));
                    }
                });
            });
        action
    }

    fn smoke(&mut self, ctx: &egui::Context) {
        if self.started.elapsed() > Duration::from_secs(100) {
            panic!("Tab smoke timed out at step {}", self.smoke_step);
        }
        let app = &self.tabs[self.active].app;
        if app.info.is_some() && (app.busy || app.visible_missing > 0)
            || app.smoke_double_click.is_some()
        {
            ctx.request_repaint_after(Duration::from_millis(16));
            return;
        }
        self.smoke_frames += 1;
        if self.smoke_frames < 8 {
            ctx.request_repaint();
            return;
        }
        let key = |key, modifiers| egui::Event::Key {
            key,
            modifiers,
            physical_key: None,
            pressed: true,
            repeat: false,
        };
        match self.smoke_step {
            0 => {
                if app.info.is_none() {
                    ctx.request_repaint();
                    return;
                }
                self.tabs[0].app.goto(2);
                self.tabs[0].app.zoom = Zoom::FitPage;
            }
            1 => {
                let app = &mut self.tabs[0].app;
                let rect = app.smoke_hits["selection-page-2"];
                app.smoke_double_click = Some((
                    rect.min
                        + Vec2::new(rect.width() * 150.0 / 612.0, rect.height() * 72.0 / 792.0),
                    42,
                ));
            }
            2 => {
                let app = &mut self.tabs[0].app;
                assert!(
                    app.selection
                        .current
                        .as_ref()
                        .is_some_and(|s| s.text.trim() == "page")
                );
                app.click_test_control("highlight-selection");
            }
            3 => {
                assert_eq!(self.tabs[0].app.highlight_history.active().len(), 1);
                assert!(
                    self.tabs[0].app.highlight_pending.is_some(),
                    "Switch test must begin while a save is pending"
                );
                self.add(ctx, Some(PathBuf::from("tests/fixtures/grid-demo.pdf")));
            }
            4 => {
                let app = &mut self.tabs[self.active].app;
                if app.info.is_none() {
                    ctx.request_repaint();
                    return;
                }
                assert_eq!(app.pages(), 16);
                app.goto(10);
                app.rotation = 1;
                app.zoom = Zoom::Scale(0.45);
                app.open_search();
                app.ai.open_question();
            }
            5 => {
                self.tabs[self.active].app.smoke_click = Some(self.controls["tab-1"].center());
            }
            6 => {
                assert_eq!(self.active, 0);
                assert_eq!(self.tabs[0].app.page, 2);
                assert_eq!(self.tabs[0].app.highlight_history.active().len(), 1);
                self.tabs[0].app.smoke_events = vec![key(egui::Key::Z, egui::Modifiers::CTRL)];
            }
            7 => {
                assert!(self.tabs[0].app.highlight_history.active().is_empty());
                self.tabs[0].app.smoke_click = Some(self.controls["tab-2"].center());
            }
            8 => {
                assert_eq!(self.active, 1);
                assert_eq!(self.tabs[1].app.page, 10);
                assert_eq!(self.tabs[1].app.rotation, 1);
                assert!(self.tabs[1].app.search.visible && self.tabs[1].app.ai.visible);
                assert!(!self.tabs[0].app.search.visible && !self.tabs[0].app.ai.visible);
                assert!(matches!(self.tabs[1].app.zoom, Zoom::Scale(z) if z == 0.45));
                self.tabs[1].app.smoke_events = vec![key(
                    egui::Key::Tab,
                    egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                )];
            }
            9 => {
                assert_eq!(self.active, 0);
                if self.tabs[0].app.highlight_pending.is_some() {
                    ctx.request_repaint_after(Duration::from_millis(16));
                    return;
                }
                assert!(self.tabs[0].app.highlight_history.active().is_empty());
                self.tabs[0].app.smoke_events = vec![key(egui::Key::Y, egui::Modifiers::CTRL)];
            }
            10 => {
                assert_eq!(self.tabs[0].app.highlight_history.active().len(), 1);
                self.tabs[0].app.smoke_events = vec![key(egui::Key::Tab, egui::Modifiers::CTRL)];
            }
            11 => {
                assert_eq!(self.active, 1);
                assert_eq!(self.tabs[1].app.page, 10);
                self.tabs[1].app.smoke_events = vec![key(egui::Key::T, egui::Modifiers::CTRL)];
            }
            12 => {
                assert_eq!(self.tabs.len(), 3);
                assert!(self.tabs[self.active].app.path.is_none());
                self.tabs[self.active].app.smoke_events =
                    vec![key(egui::Key::W, egui::Modifiers::CTRL)];
            }
            13 => {
                assert_eq!(self.tabs.len(), 2);
                assert_eq!(self.active, 1);
                self.tabs[1].app.smoke_click = Some(self.controls["close-2"].center());
            }
            14 => {
                assert_eq!(self.tabs.len(), 1);
                assert_eq!(self.tabs[0].app.page, 2);
                assert_eq!(self.tabs[0].app.highlight_history.active().len(), 1);
                self.add(ctx, Some(PathBuf::from("tests/fixtures/grid-demo.pdf")));
            }
            15 => {
                if self.tabs[self.active].app.info.is_none() {
                    ctx.request_repaint();
                    return;
                }
                assert_eq!(self.tabs[self.active].app.pages(), 16);
                self.tabs[self.active].app.smoke_click = Some(self.controls["tab-1"].center());
            }
            16 => {
                assert_eq!(self.active, 0);
                assert_eq!(self.tabs.len(), 2);
                assert_eq!(self.tabs[0].app.page, 2);
                if self.tabs[0].app.highlight_pending.is_some() {
                    ctx.request_repaint();
                    return;
                }
                println!(
                    "Tabs verified: actual tab/close clicks, Ctrl+T/W/Tab/Shift+Tab, independent document navigation and undo, switching during background save, closing and reopening clients"
                );
                self.smoke_tabs = false;
                let app = &mut self.tabs[0].app;
                app.smoke_tabs = false;
                app.smoke_features = false;
                app.started = Instant::now();
                app.settled_frames = 0;
            }
            _ => unreachable!(),
        }
        self.smoke_step += 1;
        self.smoke_frames = 0;
        ctx.request_repaint();
    }
}

impl App for WindowApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        self.tabs[self.active].app.raw_input_hook(ctx, input);
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        for index in 0..self.tabs.len() {
            if index != self.active {
                let app = &mut self.tabs[index].app;
                app.poll_background(ctx);
                app.cache
                    .retain(|entry| entry.target.page == app.page && !entry.target.thumbnail);
                app.cache.truncate(1);
            }
        }
        if ctx.input(|i| i.viewport().close_requested())
            && let Some(index) = self.tabs.iter_mut().position(|tab| !Self::can_close(tab))
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.switch(ctx, index);
        }
        let mut action = self.tab_bar(ctx);
        let app = &mut self.tabs[self.active].app;
        if app.outline_editor.is_none()
            && app.inline_rename.is_none()
            && !app.rgb_window
            && !app.highlight_palette_open
            && !app.uninstall_confirm
            && ctx.memory(|m| m.top_modal_layer().is_none())
        {
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::CTRL, egui::Key::T) {
                    action = Some((TabAction::New, None));
                }
                if i.consume_key(egui::Modifiers::CTRL, egui::Key::W) {
                    action = Some((TabAction::Close, None));
                }
                let previous = i.consume_key(
                    egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                    egui::Key::Tab,
                );
                let next = i.consume_key(egui::Modifiers::CTRL, egui::Key::Tab);
                if previous || next {
                    let index = if previous {
                        (self.active + self.tabs.len() - 1) % self.tabs.len()
                    } else {
                        (self.active + 1) % self.tabs.len()
                    };
                    action = Some((TabAction::Switch(index), None));
                }
            });
        }
        let other_pending = self.tabs.iter().enumerate().any(|(i, t)| {
            i != self.active
                && (t.app.highlight_pending.is_some()
                    || t.app.inline_rename.is_some()
                    || t.app.outline_editor.is_some())
        });
        let app = &mut self.tabs[self.active].app;
        app.other_tab_pending = other_pending;
        app.update(ctx, frame);
        let paths = std::mem::take(&mut app.pending_tab_paths);
        if let Some(menu_action) = app.tab_action.take() {
            action = Some((menu_action, None));
        }
        if let Some((action, index)) = action {
            match action {
                TabAction::New => self.add(ctx, None),
                TabAction::Close => self.close(ctx, index.unwrap_or(self.active)),
                TabAction::Switch(index) => self.switch(ctx, index),
            }
        }
        for path in paths {
            self.add(ctx, Some(path));
        }
        if self.smoke_tabs {
            self.smoke(ctx);
        }
    }
}
