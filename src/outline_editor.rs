use eframe::egui::{self, RichText};
use fpdf::outline::{OutlineDocument, Tree};
use fpdf::pdf::Bookmark;
use std::path::PathBuf;
use std::sync::mpsc;

enum Reply {
    Loaded(Tree, u32),
    Saved(PathBuf),
    Error(String),
}
pub enum Outcome {
    Keep,
    Close,
    Saved(PathBuf),
}
pub struct OutlineEditor {
    source: PathBuf,
    tree: Tree,
    original: Tree,
    selected: Option<usize>,
    history: Vec<Tree>,
    commands: mpsc::Sender<(Tree, PathBuf)>,
    replies: mpsc::Receiver<Reply>,
    loading: bool,
    saving: bool,
    pages: u32,
    error: Option<String>,
    pub hits: std::collections::HashMap<String, egui::Rect>,
}
impl OutlineEditor {
    pub fn entries(&self) -> &[fpdf::outline::Entry] {
        &self.tree.entries
    }
    pub fn guard_close(&mut self) -> bool {
        if self.saving || self.tree.entries != self.original.entries {
            self.error = Some("目录有未保存修改或正在保存，请保存或放弃修改后再关闭程序。".into());
            true
        } else {
            false
        }
    }
    pub fn new(ctx: &egui::Context, path: PathBuf, bookmarks: Vec<Bookmark>) -> Self {
        let (commands, receiver) = mpsc::channel::<(Tree, PathBuf)>();
        let (sender, replies) = mpsc::channel();
        let context = ctx.clone();
        let source = path.clone();
        std::thread::spawn(move || {
            let document = match OutlineDocument::load(&source, &bookmarks) {
                Ok(doc) => doc,
                Err(e) => {
                    let _ = sender.send(Reply::Error(e));
                    context.request_repaint();
                    return;
                }
            };
            if sender
                .send(Reply::Loaded(document.tree.clone(), document.pages))
                .is_err()
            {
                return;
            }
            context.request_repaint();
            while let Ok((tree, path)) = receiver.recv() {
                let reply = match document.save(&tree, &path) {
                    Ok(()) => Reply::Saved(path),
                    Err(e) => Reply::Error(e),
                };
                if sender.send(reply).is_err() {
                    return;
                }
                context.request_repaint();
            }
        });
        Self {
            source: path,
            tree: Tree::default(),
            original: Tree::default(),
            selected: None,
            history: Vec::new(),
            commands,
            replies,
            loading: true,
            saving: false,
            pages: 0,
            error: None,
            hits: Default::default(),
        }
    }
    fn hit(&mut self, name: &str, response: egui::Response) -> egui::Response {
        self.hits.insert(name.into(), response.rect);
        response
    }
    fn save(&mut self, path: PathBuf) {
        if self.commands.send((self.tree.clone(), path)).is_err() {
            self.error = Some("目录编辑线程已退出，请关闭编辑器后重试。".into());
        } else {
            self.saving = true;
            self.error = None;
        }
    }
    pub fn is_ready(&self) -> bool {
        !self.loading && !self.saving && self.pages > 0
    }
    pub fn show(&mut self, ctx: &egui::Context, current_page: u32) -> Outcome {
        while let Ok(reply) = self.replies.try_recv() {
            match reply {
                Reply::Loaded(tree, pages) => {
                    self.original = tree.clone();
                    self.tree = tree;
                    self.pages = pages;
                    self.selected = (!self.tree.entries.is_empty()).then_some(0);
                    self.loading = false;
                }
                Reply::Saved(path) => return Outcome::Saved(path),
                Reply::Error(e) => {
                    self.loading = false;
                    self.saving = false;
                    self.error = Some(e);
                }
            }
        }
        let mut outcome = Outcome::Keep;
        let before = self.tree.clone();
        let mut undo = false;
        egui::Modal::new(egui::Id::new("outline-editor")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 70.0).clamp(400.0, 820.0));
            ui.heading("编辑文档目录");
            ui.label(
                self.source
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy(),
            );
            ui.label("选择章节后修改名称或跳转页码；保存后写入 PDF，重新打开仍然有效。");
            ui.separator();
            if self.loading || self.saving {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(if self.saving {
                        "正在保存目录…"
                    } else {
                        "正在载入目录…"
                    });
                });
            }
            if let Some(error) = &self.error {
                ui.colored_label(egui::Color32::from_rgb(210, 60, 60), error);
            }
            ui.add_enabled_ui(self.is_ready(), |ui| {
                ui.horizontal_wrapped(|ui| {
                    if self.hit("add", ui.button("新增章节")).clicked() {
                        self.selected = Some(self.tree.add(self.selected, false, current_page));
                    }
                    if self
                        .hit(
                            "add-child",
                            ui.add_enabled(
                                self.selected
                                    .is_some_and(|i| self.tree.entries[i].depth < 32),
                                egui::Button::new("新增子章节"),
                            ),
                        )
                        .clicked()
                    {
                        self.selected = Some(self.tree.add(self.selected, true, current_page));
                    }
                    if self
                        .hit(
                            "undo",
                            ui.add_enabled(!self.history.is_empty(), egui::Button::new("撤销")),
                        )
                        .clicked()
                    {
                        undo = true;
                    }
                    ui.label(RichText::new(format!("{} 项", self.tree.entries.len())).small());
                });
                egui::ScrollArea::vertical()
                    .id_salt("editable-outline")
                    .max_height((ctx.content_rect().height() - 410.0).clamp(100.0, 320.0))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        for i in 0..self.tree.entries.len() {
                            let e = &self.tree.entries[i];
                            let text = format!(
                                "{}{}{}",
                                "    ".repeat(e.depth as usize),
                                e.title,
                                e.page.map_or("  · 无跳转".into(), |p| format!(
                                    "  · 第 {} 页",
                                    p + 1
                                ))
                            );
                            if self
                                .hit(
                                    &format!("entry-{i}"),
                                    ui.selectable_label(self.selected == Some(i), text),
                                )
                                .clicked()
                            {
                                self.selected = Some(i);
                            }
                        }
                        if self.tree.entries.is_empty() {
                            ui.label("暂无目录，点击“新增章节”创建。");
                        }
                    });
                ui.separator();
                if let Some(index) = self.selected.filter(|&i| i < self.tree.entries.len()) {
                    ui.horizontal(|ui| {
                        ui.label("名称");
                        let response = ui.add_sized(
                            [ui.available_width(), 27.0],
                            egui::TextEdit::singleline(&mut self.tree.entries[index].title)
                                .char_limit(2048),
                        );
                        self.hit("title", response);
                    });
                    ui.horizontal(|ui| {
                        let mut target = self.tree.entries[index].page.is_some();
                        if ui.checkbox(&mut target, "跳转页码").changed() {
                            self.tree.entries[index].page = target.then_some(current_page);
                        }
                        if let Some(page) = self.tree.entries[index].page {
                            let mut number = page + 1;
                            let response = ui.add(
                                egui::DragValue::new(&mut number)
                                    .range(1..=self.pages)
                                    .speed(0.1),
                            );
                            if self.hit("page", response).changed() {
                                self.tree.entries[index].page = Some(number - 1);
                            }
                        }
                        if self.hit("current-page", ui.button("设为当前页")).clicked() {
                            self.tree.entries[index].page = Some(current_page);
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        if self.hit("up", ui.button("上移")).clicked() {
                            self.selected = Some(self.tree.move_sibling(index, false));
                        }
                        if self.hit("down", ui.button("下移")).clicked() {
                            self.selected = Some(self.tree.move_sibling(index, true));
                        }
                        if self
                            .hit("indent", ui.button("降一级"))
                            .on_hover_text("成为前一章节的子章节")
                            .clicked()
                            && let Some(index) = self.tree.indent(self.selected.unwrap(), true)
                        {
                            self.selected = Some(index);
                        }
                        if self
                            .hit("outdent", ui.button("升一级"))
                            .on_hover_text("提升到父章节之后，子项随章节一起移动")
                            .clicked()
                            && let Some(index) = self.tree.indent(self.selected.unwrap(), false)
                        {
                            self.selected = Some(index);
                        }
                        if self
                            .hit("delete", ui.button("删除章节（含子项）"))
                            .clicked()
                        {
                            self.tree.remove(self.selected.unwrap());
                            self.selected = (!self.tree.entries.is_empty())
                                .then_some(index.min(self.tree.entries.len().saturating_sub(1)));
                        }
                    });
                }
                ui.separator();
                let dirty = self.tree.entries != self.original.entries;
                ui.horizontal(|ui| {
                    if self
                        .hit(
                            "save",
                            ui.add_enabled(dirty, egui::Button::new("保存到原 PDF")),
                        )
                        .clicked()
                    {
                        self.save(self.source.clone());
                    }
                    if self.hit("save-as", ui.button("另存为…")).clicked() {
                        let stem = self
                            .source
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy();
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PDF 文档", &["pdf"])
                            .set_directory(
                                self.source.parent().unwrap_or(std::path::Path::new(".")),
                            )
                            .set_file_name(format!("{stem}-目录.pdf"))
                            .save_file()
                        {
                            self.save(path);
                        }
                    }
                    ui.label(if dirty {
                        "有未保存修改"
                    } else {
                        "目录未修改"
                    });
                });
            });
            if self
                .hit(
                    "close",
                    ui.add_enabled(
                        !self.saving,
                        egui::Button::new(if self.tree.entries != self.original.entries {
                            "放弃修改并关闭"
                        } else {
                            "关闭"
                        }),
                    ),
                )
                .clicked()
            {
                outcome = Outcome::Close;
            }
        });
        if undo {
            if let Some(previous) = self.history.pop() {
                self.tree = previous;
                self.selected = (!self.tree.entries.is_empty()).then_some(0);
            }
        } else if before.entries != self.tree.entries && !self.loading && self.pages > 0 {
            self.history.push(before);
            if self.history.len() > 32 {
                self.history.remove(0);
            }
        }
        outcome
    }
}
