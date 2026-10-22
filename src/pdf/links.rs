use super::*;

#[repr(C)]
#[derive(Default)]
pub(super) struct RectF {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PdfDestination {
    pub page: u32,
    /// Position in the destination page after the current viewer rotation.
    pub position: Option<[f32; 2]>,
}

#[derive(Clone, Debug)]
pub struct PdfLink {
    /// Normalized visible-page coordinates, including crop and rotation.
    pub rect: [f32; 4],
    pub destination: PdfDestination,
}

impl PdfEngine {
    pub(super) fn page_point(
        &self,
        page: Handle,
        rotation: u8,
        x: f64,
        y: f64,
    ) -> Option<[f32; 2]> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let (mut dx, mut dy) = (0, 0);
        // SAFETY: page is borrowed from a guard on this engine's owner thread.
        let ok = unsafe {
            (self.api.page_to_device)(
                page,
                0,
                0,
                10000,
                10000,
                rotation.into(),
                x,
                y,
                &mut dx,
                &mut dy,
            )
        };
        (ok != 0).then_some([dx as f32 / 10000.0, dy as f32 / 10000.0])
    }

    fn destination(&self, doc: &Document, dest: Handle, rotation: u8) -> Option<PdfDestination> {
        // SAFETY: destination and document are live, thread-confined PDFium handles.
        unsafe {
            let index = (self.api.dest_page)(doc.handle, dest);
            if index < 0 || index as u32 >= doc.pages {
                return None;
            }
            let mut count = 0;
            let mut params = [0.0f32; 4];
            let view = (self.api.dest_view)(dest, &mut count, params.as_mut_ptr());
            let (mut hx, mut hy, mut hz) = (0, 0, 0);
            let (mut x, mut y, mut zoom) = (0.0, 0.0, 0.0);
            let xyz = (self.api.dest_location)(
                dest, &mut hx, &mut hy, &mut hz, &mut x, &mut y, &mut zoom,
            );
            let point = if xyz != 0 && hy != 0 {
                Some([if hx != 0 { x } else { 0.0 }, y])
            } else if (view == 3 || view == 7) && count >= 1 {
                Some([0.0, params[0]])
            } else if view == 5 && count == 4 {
                Some([params[0], params[3]])
            } else {
                None
            };
            let position = point.and_then(|[x, y]| {
                let page = (self.api.load_page)(doc.handle, index);
                if page.is_null() {
                    return None;
                }
                let _page = Guard {
                    handle: page,
                    close: self.api.close_page,
                };
                self.page_point(page, rotation, x.into(), y.into())
            });
            Some(PdfDestination {
                page: index as u32,
                position,
            })
        }
    }

    pub(super) fn page_links(&self, doc: &Document, page: Handle, rotation: u8) -> Vec<PdfLink> {
        let mut links = Vec::new();
        let mut start = 0;
        // SAFETY: native links are used only while the source page is alive;
        // only numeric rectangles/destinations leave this method.
        unsafe {
            for _ in 0..4096 {
                let mut link = std::ptr::null_mut();
                if (self.api.link_enumerate)(page, &mut start, &mut link) == 0 {
                    break;
                }
                if link.is_null() {
                    continue;
                }
                let mut dest = (self.api.link_dest)(doc.handle, link);
                if dest.is_null() {
                    let action = (self.api.link_action)(link);
                    if !action.is_null() && (self.api.action_type)(action) == 1 {
                        dest = (self.api.action_dest)(doc.handle, action);
                    }
                }
                if dest.is_null() {
                    continue;
                }
                let Some(destination) = self.destination(doc, dest, rotation) else {
                    continue;
                };
                let mut r = RectF::default();
                if (self.api.link_rect)(link, &mut r) == 0 {
                    continue;
                }
                let mut rect = [
                    f32::INFINITY,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                    f32::NEG_INFINITY,
                ];
                for (x, y) in [
                    (r.left, r.top),
                    (r.right, r.top),
                    (r.left, r.bottom),
                    (r.right, r.bottom),
                ] {
                    if let Some([x, y]) = self.page_point(page, rotation, x.into(), y.into()) {
                        rect[0] = rect[0].min(x);
                        rect[1] = rect[1].min(y);
                        rect[2] = rect[2].max(x);
                        rect[3] = rect[3].max(y);
                    }
                }
                if rect.iter().all(|v| v.is_finite()) && rect[2] > rect[0] && rect[3] > rect[1] {
                    links.push(PdfLink { rect, destination });
                }
            }
        }
        links
    }
}
