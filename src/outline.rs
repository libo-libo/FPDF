//! Outline editing preserves existing actions, styles, and unrelated PDF objects.
use crate::pdf::Bookmark;
use lopdf::{Dictionary, Document, Object, ObjectId, StringFormat};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub source: Option<ObjectId>,
    pub title: String,
    pub page: Option<u32>,
    pub depth: u8,
}

#[derive(Clone, Default)]
pub struct Tree {
    pub entries: Vec<Entry>,
}
impl Tree {
    pub fn end(&self, index: usize) -> usize {
        let depth = self.entries[index].depth;
        (index + 1..self.entries.len())
            .find(|&i| self.entries[i].depth <= depth)
            .unwrap_or(self.entries.len())
    }
    pub fn add(&mut self, selected: Option<usize>, child: bool, page: u32) -> usize {
        let (at, depth) =
            selected
                .filter(|&i| i < self.entries.len())
                .map_or((self.entries.len(), 0), |i| {
                    (
                        self.end(i),
                        (self.entries[i].depth + u8::from(child)).min(32),
                    )
                });
        self.entries.insert(
            at,
            Entry {
                source: None,
                title: "新章节".into(),
                page: Some(page),
                depth,
            },
        );
        at
    }
    pub fn remove(&mut self, index: usize) {
        let end = self.end(index);
        self.entries.drain(index..end);
    }
    /// Move complete subtrees among siblings, preserving every descendant.
    pub fn move_sibling(&mut self, index: usize, down: bool) -> usize {
        let depth = self.entries[index].depth;
        let end = self.end(index);
        if down {
            if end < self.entries.len() && self.entries[end].depth == depth {
                let next_end = self.end(end);
                self.entries[index..next_end].rotate_left(end - index);
                return index + next_end - end;
            }
        } else if let Some(previous) = (0..index).rev().find(|&i| self.entries[i].depth <= depth)
            && self.entries[previous].depth == depth
        {
            self.entries[previous..end].rotate_right(end - index);
            return previous;
        }
        index
    }
    pub fn indent(&mut self, index: usize, deeper: bool) -> Option<usize> {
        let depth = self.entries[index].depth;
        if deeper && (depth >= 32 || index == 0 || self.entries[index - 1].depth < depth) {
            return None;
        }
        if !deeper && depth == 0 {
            return None;
        }
        let end = self.end(index);
        if deeper && self.entries[index..end].iter().any(|e| e.depth >= 32) {
            return None;
        }
        let at = if deeper {
            index
        } else {
            let parent = (0..index).rev().find(|&i| self.entries[i].depth < depth)?;
            let parent_end = self.end(parent);
            self.entries[index..parent_end].rotate_left(end - index);
            parent_end - (end - index)
        };
        for entry in &mut self.entries[at..at + end - index] {
            entry.depth = if deeper {
                entry.depth + 1
            } else {
                entry.depth - 1
            };
        }
        Some(at)
    }
}

pub struct OutlineDocument {
    document: Document,
    source: PathBuf,
    original: Vec<u8>,
    pub tree: Tree,
    pub pages: u32,
}
fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(512 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 512 * 1024 * 1024 {
        return Err("文件超过 512 MiB。".into());
    }
    Ok(bytes)
}
impl OutlineDocument {
    pub fn load(path: &Path, bookmarks: &[Bookmark]) -> Result<Self, String> {
        let original = read_bounded(path)?;
        let document =
            Document::load_mem(&original).map_err(|e| format!("无法载入目录编辑器：{e}"))?;
        if document.is_encrypted() || document.was_encrypted() {
            return Err("加密 PDF 暂不支持修改目录，原文件未改变。".into());
        }
        if document.objects.values().any(|o| {
            o.as_dict().is_ok_and(|d| {
                d.get(b"Type")
                    .and_then(Object::as_name)
                    .is_ok_and(|n| n == b"Sig")
                    || (d.has(b"ByteRange") && d.has(b"Contents"))
            })
        }) {
            return Err("此 PDF 含数字签名，修改目录会使签名失效，当前版本不保存此类文档。".into());
        }
        let pages = document.get_pages().len() as u32;
        let mut ids = Vec::new();
        let mut seen = HashSet::new();
        if let Ok(root) = document
            .catalog()
            .map_err(|e| e.to_string())?
            .get(b"Outlines")
        {
            let (_, root) = document.dereference(root).map_err(|e| e.to_string())?;
            if let Ok(first) = root.as_dict().map_err(|e| e.to_string())?.get(b"First") {
                collect_ids(
                    &document,
                    first.as_reference().map_err(|e| e.to_string())?,
                    0,
                    &mut ids,
                    &mut seen,
                )?;
            }
        }
        if ids.len() != bookmarks.len()
            || ids
                .iter()
                .zip(bookmarks)
                .any(|((_, depth), b)| *depth != b.depth)
        {
            return Err("目录结构无法完整识别，已停止编辑以保留原有目录。".into());
        }
        let tree = Tree {
            entries: ids
                .into_iter()
                .zip(bookmarks)
                .map(|((id, depth), b)| Entry {
                    source: Some(id),
                    title: b.title.clone(),
                    page: b.page,
                    depth,
                })
                .collect(),
        };
        Ok(Self {
            document,
            source: path.to_owned(),
            original,
            tree,
            pages,
        })
    }
    pub fn save(&self, tree: &Tree, output: &Path) -> Result<(), String> {
        if tree.entries.len() > 5000 {
            return Err("目录最多支持 5,000 项。".into());
        }
        let mut previous_depth = 0;
        let mut seen = HashSet::new();
        for (i, e) in tree.entries.iter().enumerate() {
            if e.title.trim().is_empty()
                || e.title.len() > 8192
                || e.depth > 32
                || (i == 0 && e.depth != 0)
                || e.depth > previous_depth + 1
                || e.page.is_some_and(|p| p >= self.pages)
                || e.source.is_some_and(|id| {
                    !seen.insert(id) || !self.tree.entries.iter().any(|e| e.source == Some(id))
                })
            {
                return Err("请检查目录名称、层级和目标页码。".into());
            }
            previous_depth = e.depth;
        }
        // Saving an outdated editor must never replace a newer source file.
        if read_bounded(&self.source)? != self.original {
            return Err("源 PDF 已被其他程序修改。请关闭编辑器并重新打开文件后再编辑。".into());
        }
        let mut doc = self.document.clone();
        let page_ids: Vec<_> = doc.get_pages().into_values().collect();
        let old_root = doc
            .catalog()
            .map_err(|e| e.to_string())?
            .get(b"Outlines")
            .ok()
            .and_then(|v| v.as_reference().ok());
        let root_id = old_root.unwrap_or_else(|| doc.add_object(Dictionary::new()));
        let ids: Vec<_> = tree
            .entries
            .iter()
            .map(|e| {
                e.source
                    .unwrap_or_else(|| doc.add_object(Dictionary::new()))
            })
            .collect();
        let mut children = vec![Vec::new(); ids.len() + 1];
        let mut ancestors = Vec::new();
        for (i, entry) in tree.entries.iter().enumerate() {
            ancestors.truncate(entry.depth as usize);
            let parent = ancestors.last().copied().unwrap_or(ids.len());
            children[parent].push(i);
            ancestors.push(i);
        }
        for (i, entry) in tree.entries.iter().enumerate() {
            let mut dict = entry
                .source
                .and_then(|id| doc.get_dictionary(id).ok())
                .cloned()
                .unwrap_or_default();
            for key in [
                b"Parent".as_slice(),
                b"Prev",
                b"Next",
                b"First",
                b"Last",
                b"Count",
            ] {
                dict.remove(key);
            }
            let title: Vec<u8> = [0xfe, 0xff]
                .into_iter()
                .chain(entry.title.encode_utf16().flat_map(u16::to_be_bytes))
                .collect();
            dict.set("Title", Object::String(title, StringFormat::Hexadecimal));
            let original_page = self
                .tree
                .entries
                .iter()
                .find(|e| e.source == entry.source && e.source.is_some())
                .and_then(|e| e.page);
            if entry.source.is_none() || entry.page != original_page {
                dict.remove(b"A");
                dict.remove(b"Dest");
                if let Some(page) = entry.page {
                    dict.set(
                        "Dest",
                        Object::Array(vec![
                            Object::Reference(page_ids[page as usize]),
                            Object::Name(b"Fit".to_vec()),
                        ]),
                    );
                }
            }
            doc.objects.insert(ids[i], Object::Dictionary(dict));
        }
        for (parent, siblings) in children.iter().enumerate() {
            let parent_id = ids.get(parent).copied().unwrap_or(root_id);
            if let (Some(first), Some(last)) = (siblings.first(), siblings.last()) {
                let dict = doc
                    .get_dictionary_mut(parent_id)
                    .map_err(|e| e.to_string())?;
                dict.set("First", ids[*first]);
                dict.set("Last", ids[*last]);
                let count = if parent == ids.len() {
                    ids.len()
                } else {
                    tree.end(parent) - parent - 1
                };
                dict.set("Count", count as i64);
            }
            for (at, &index) in siblings.iter().enumerate() {
                let dict = doc
                    .get_dictionary_mut(ids[index])
                    .map_err(|e| e.to_string())?;
                dict.set("Parent", parent_id);
                if at > 0 {
                    dict.set("Prev", ids[siblings[at - 1]]);
                }
                if at + 1 < siblings.len() {
                    dict.set("Next", ids[siblings[at + 1]]);
                }
            }
        }
        if ids.is_empty() {
            doc.catalog_mut()
                .map_err(|e| e.to_string())?
                .remove(b"Outlines");
        } else {
            doc.catalog_mut()
                .map_err(|e| e.to_string())?
                .set("Outlines", root_id);
        }
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temp = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| format!("无法创建保存文件：{e}"))?;
        doc.save_to(temp.as_file_mut())
            .map_err(|e| format!("写入失败，原文件未改变：{e}"))?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        // Validate the serialized PDF before the atomic replacement.
        let check = Document::load(temp.path()).map_err(|e| format!("保存验证失败：{e}"))?;
        if check.get_pages().len() != page_ids.len() {
            return Err("保存后页数不一致，原文件未改变。".into());
        }
        temp.persist(output)
            .map_err(|e| format!("无法替换目标文件，原文件未改变：{}", e.error))?;
        Ok(())
    }
}
fn collect_ids(
    doc: &Document,
    mut id: ObjectId,
    depth: u8,
    result: &mut Vec<(ObjectId, u8)>,
    seen: &mut HashSet<ObjectId>,
) -> Result<(), String> {
    loop {
        if depth > 32 || result.len() >= 5000 || !seen.insert(id) {
            return Err("目录过深、过多或包含循环，无法编辑。".into());
        }
        result.push((id, depth));
        let dict = doc.get_dictionary(id).map_err(|e| e.to_string())?;
        if let Ok(child) = dict.get(b"First") {
            collect_ids(
                doc,
                child.as_reference().map_err(|e| e.to_string())?,
                depth + 1,
                result,
                seen,
            )?;
        }
        match dict.get(b"Next") {
            Ok(next) => id = next.as_reference().map_err(|e| e.to_string())?,
            Err(_) => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tree() -> Tree {
        Tree {
            entries: [
                ("a", 0),
                ("a1", 1),
                ("a2", 1),
                ("b", 0),
                ("b1", 1),
                ("c", 0),
            ]
            .into_iter()
            .map(|(s, depth)| Entry {
                source: None,
                title: s.into(),
                page: Some(0),
                depth,
            })
            .collect(),
        }
    }
    #[test]
    fn sibling_moves_keep_children_and_outdent_preserves_following_siblings() {
        let mut t = tree();
        assert_eq!(t.move_sibling(0, true), 2);
        assert_eq!(
            t.entries
                .iter()
                .map(|e| e.title.as_str())
                .collect::<Vec<_>>(),
            ["b", "b1", "a", "a1", "a2", "c"]
        );
        assert_eq!(t.move_sibling(2, false), 0);
        assert_eq!(t.indent(1, false), Some(2));
        assert_eq!(
            t.entries
                .iter()
                .map(|e| (e.title.as_str(), e.depth))
                .collect::<Vec<_>>(),
            [
                ("a", 0),
                ("a2", 1),
                ("a1", 0),
                ("b", 0),
                ("b1", 1),
                ("c", 0)
            ]
        );
        assert_eq!(t.indent(2, true), Some(2));
        assert_eq!(t.entries[2].depth, 1);
        t.remove(0);
        assert_eq!(t.entries[0].title, "b");
        assert_eq!(t.entries.len(), 3);
        assert_eq!(t.indent(0, true), None);
        assert_eq!(t.indent(0, false), None);
    }
}
