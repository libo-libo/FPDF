use fpdf::outline::OutlineDocument;
use fpdf::pdf::Bookmark;
use std::path::PathBuf;
use std::sync::mpsc;

pub struct Rename {
    pub index: usize,
    pub original: String,
    pub title: String,
    pub focus: bool,
    pub error: Option<String>,
    pub result: Option<mpsc::Receiver<Result<(), String>>>,
}
impl Rename {
    pub fn new(index: usize, title: String) -> Self {
        Self {
            index,
            original: title.clone(),
            title,
            focus: true,
            error: None,
            result: None,
        }
    }
    pub fn changed(&self) -> bool {
        self.title != self.original
    }
    pub fn save(&mut self, ctx: &eframe::egui::Context, path: PathBuf, bookmarks: Vec<Bookmark>) {
        if self.result.is_some() {
            return;
        }
        if self.title.trim().is_empty() {
            self.error = Some("目录名称不能为空。".into());
            self.focus = true;
            return;
        }
        let index = self.index;
        let title = self.title.clone();
        let (sender, result) = mpsc::channel();
        self.result = Some(result);
        self.error = None;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let saved = (|| {
                let doc = OutlineDocument::load(&path, &bookmarks)?;
                let mut tree = doc.tree.clone();
                tree.entries
                    .get_mut(index)
                    .ok_or("目录已改变，请重试。")?
                    .title = title;
                doc.save(&tree, &path)
            })();
            let _ = sender.send(saved);
            ctx.request_repaint();
        });
    }
}
