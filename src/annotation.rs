//! Standards-compatible text highlights with atomic, identity-checked persistence.
use crate::pdf::PdfSelection;
use lopdf::{Document, Object, Stream, StringFormat, dictionary};
use std::io::Read;
use std::path::{Path, PathBuf};

pub struct HighlightDocument {
    document: Document,
    source: PathBuf,
    original: Vec<u8>,
}
fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(512 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 512 * 1024 * 1024 {
        return Err("PDF 超过 512 MiB。".into());
    }
    Ok(bytes)
}
// Compare the opened snapshot without allocating another potentially 512 MiB file.
fn unchanged(path: &Path, expected: &[u8]) -> Result<bool, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buffer = [0u8; 65536];
    let mut offset = 0usize;
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            return Ok(offset == expected.len());
        }
        let Some(end) = offset.checked_add(count) else {
            return Ok(false);
        };
        if end > expected.len() || expected[offset..end] != buffer[..count] {
            return Ok(false);
        }
        offset = end;
    }
}
impl HighlightDocument {
    pub fn load(path: &Path) -> Result<Self, String> {
        let original = read_bounded(path)?;
        let document = Document::load_mem(&original).map_err(|e| format!("无法读取 PDF：{e}"))?;
        if document.is_encrypted() || document.was_encrypted() {
            return Err("加密 PDF 暂不支持保存高亮。".into());
        }
        if document.objects.values().any(|o| {
            o.as_dict().is_ok_and(|d| {
                d.get(b"Type")
                    .and_then(Object::as_name)
                    .is_ok_and(|n| n == b"Sig")
                    || (d.has(b"ByteRange") && d.has(b"Contents"))
            })
        }) {
            return Err("此 PDF 含数字签名，保存高亮会使签名失效，已停止保存。".into());
        }
        Ok(Self {
            document,
            source: path.to_owned(),
            original,
        })
    }
    pub fn matches_source(&self, bytes: &[u8]) -> bool {
        self.original == bytes
    }

    pub fn add_selection(
        &mut self,
        selection: &PdfSelection,
        _rotation: u8,
        color: [u8; 3],
    ) -> Result<(), String> {
        if selection.quads.is_empty()
            || selection.quads.len() > 256
            || selection
                .quads
                .iter()
                .flatten()
                .any(|x| !x.is_finite() || x.abs() > 10_000_000.0)
        {
            return Err("选区没有可保存的文字坐标。".into());
        }
        let page = *self
            .document
            .get_pages()
            .get(&selection.page.checked_add(1).ok_or("高亮页码无效。")?)
            .ok_or("高亮页码无效。")?;
        let mut annots = match self
            .document
            .get_dictionary(page)
            .map_err(|e| e.to_string())?
            .get(b"Annots")
        {
            Ok(object) => self
                .document
                .dereference(object)
                .map_err(|e| e.to_string())?
                .1
                .as_array()
                .map_err(|e| e.to_string())?
                .clone(),
            Err(_) => Vec::new(),
        };
        let (mut left, mut bottom, mut right, mut top) = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for q in &selection.quads {
            for xy in q.as_chunks::<2>().0 {
                left = left.min(xy[0]);
                right = right.max(xy[0]);
                bottom = bottom.min(xy[1]);
                top = top.max(xy[1]);
            }
        }
        if right <= left || top <= bottom {
            return Err("高亮选区面积无效。".into());
        }
        let rgb = color.map(|c| f32::from(c) / 255.0);
        let mut content = format!("q /GS0 gs {} {} {} rg\n", rgb[0], rgb[1], rgb[2]);
        for q in &selection.quads {
            content.push_str(&format!(
                "{} {} m {} {} l {} {} l {} {} l h f\n",
                q[0] - left,
                q[1] - bottom,
                q[2] - left,
                q[3] - bottom,
                q[6] - left,
                q[7] - bottom,
                q[4] - left,
                q[5] - bottom
            ));
        }
        content.push_str("Q\n");
        let resources = dictionary! {"ExtGState"=>dictionary!{"GS0"=>dictionary!{"Type"=>"ExtGState","ca"=>0.35f32,"CA"=>0.35f32,"BM"=>"Multiply"}}};
        let appearance=self.document.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","FormType"=>1,"BBox"=>vec![0.into(),0.into(),Object::Real((right-left) as f32),Object::Real((top-bottom) as f32)],"Resources"=>resources},content.into_bytes()));
        let quad_points: Vec<Object> = selection
            .quads
            .iter()
            .flatten()
            .map(|v| Object::Real(*v as f32))
            .collect();
        let rect: Vec<Object> = [left, bottom, right, top]
            .into_iter()
            .map(|v| Object::Real(v as f32))
            .collect();
        let annotation=self.document.add_object(dictionary!{
            "Type"=>"Annot","Subtype"=>"Highlight","P"=>page,"Rect"=>rect,"QuadPoints"=>quad_points,
            "C"=>rgb.into_iter().map(Object::Real).collect::<Vec<_>>(),"CA"=>0.35f32,"F"=>4,
            "Contents"=>Object::String(std::iter::once(0xFEFFu16).chain(selection.text.encode_utf16()).flat_map(u16::to_be_bytes).collect(), StringFormat::Hexadecimal),"AP"=>dictionary!{"N"=>appearance},
        });
        annots.push(annotation.into());
        self.document
            .get_dictionary_mut(page)
            .map_err(|e| e.to_string())?
            .set("Annots", annots);
        Ok(())
    }
    pub fn save(&self, output: &Path) -> Result<(), String> {
        if !unchanged(&self.source, &self.original)? {
            return Err("原 PDF 已被其他程序修改，请重新打开后再保存。".into());
        }
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        let mut document = self.document.clone();
        document
            .save_to(temp.as_file_mut())
            .map_err(|e| format!("高亮保存失败：{e}"))?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        let check = Document::load(temp.path()).map_err(|e| format!("高亮保存验证失败：{e}"))?;
        if check.get_pages().len() != self.document.get_pages().len() {
            return Err("保存后的 PDF 页数发生变化。".into());
        }
        // Recheck immediately before replacement, after serialization and validation.
        if !unchanged(&self.source, &self.original)? {
            return Err("原 PDF 在保存过程中被修改，已取消保存。".into());
        }
        temp.persist(output)
            .map_err(|e| format!("无法替换目标文件：{}", e.error))?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::unchanged;

    #[test]
    fn streaming_identity_detects_content_size_and_tail_changes() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("snapshot.bin");
        let original = vec![42u8; 131_083];
        std::fs::write(&source, &original).unwrap();
        assert!(unchanged(&source, &original).unwrap());
        let mut changed = original.clone();
        changed[131_080] = 43;
        std::fs::write(&source, &changed).unwrap();
        assert!(!unchanged(&source, &original).unwrap());
        std::fs::write(&source, &original[..131_080]).unwrap();
        assert!(!unchanged(&source, &original).unwrap());
        let mut extended = original.clone();
        extended.push(0);
        std::fs::write(&source, &extended).unwrap();
        assert!(!unchanged(&source, &original).unwrap());
    }
}
