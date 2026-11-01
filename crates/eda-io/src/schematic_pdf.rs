//! Self-contained PDF 1.4 pages with lossless RGB images and navigable bookmarks.
//! Screen-space rasterization belongs to eda-render; this module only packages data.
use std::{fmt::Write as _, io::Write, path::Path};
pub struct Bookmark {
    pub title: String,
    /// PDF page coordinates, in points, origin at bottom left.
    pub rect: [f64; 4],
    pub details: String,
    /// Optional cross-page destination; the outline still points at this object.
    pub destination: Option<(usize, [f64; 4])>,
}
pub struct Page {
    pub title: String,
    pub width_pt: f64,
    pub height_pt: f64,
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<u8>,
    pub bookmarks: Vec<Bookmark>,
}
fn unicode(s: &str) -> String {
    let mut out = String::from("<FEFF");
    for n in s.encode_utf16() {
        write!(out, "{n:04X}").unwrap();
    }
    out.push('>');
    out
}
fn stream(header: &str, data: &[u8]) -> Vec<u8> {
    let mut result = format!("<< {header} /Length {} >>\nstream\n", data.len()).into_bytes();
    result.extend(data);
    result.extend(b"\nendstream");
    result
}
pub fn serialize(pages: &[Page]) -> Result<Vec<u8>, String> {
    if pages.is_empty() || pages.len() > 1000 {
        return Err("PDF needs 1–1000 pages".into());
    }
    let mut objects = vec![Vec::<u8>::new(); 3 + pages.len()]; // Catalog, Pages, Outlines, page placeholders.
    let page_ids: Vec<_> = (4..4 + pages.len()).collect();
    let mut outline_ids = Vec::new();
    for (page_index, page) in pages.iter().enumerate() {
        if page
            .width
            .checked_mul(page.height)
            .and_then(|n| n.checked_mul(3))
            != Some(page.rgb.len())
            || page.rgb.len() > 96_000_000
            || !page.width_pt.is_finite()
            || !page.height_pt.is_finite()
            || page.width_pt <= 0.
            || page.height_pt <= 0.
        {
            return Err("Invalid PDF page image or dimensions".into());
        }
        let page_id = page_ids[page_index];
        let image_id = objects.len() + 1;
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&page.rgb).map_err(|e| e.to_string())?;
        let compressed = encoder.finish().map_err(|e| e.to_string())?;
        objects.push(stream(&format!("/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode",page.width,page.height),&compressed));
        let content_id = objects.len() + 1;
        let content = format!(
            "q\n{:.5} 0 0 {:.5} 0 0 cm\n/Im0 Do\nQ",
            page.width_pt, page.height_pt
        );
        objects.push(stream("", content.as_bytes()));
        let outline_id = objects.len() + 1;
        objects.push(vec![]);
        outline_ids.push(outline_id);
        let mut child_ids = Vec::new();
        let mut annotation_ids = Vec::new();
        for mark in &page.bookmarks {
            if !mark.rect.iter().all(|v| v.is_finite()) {
                return Err("Invalid PDF bookmark coordinates".into());
            }
            let id = objects.len() + 1;
            objects.push(vec![]);
            child_ids.push(id);
            let annotation = objects.len() + 1;
            annotation_ids.push(annotation);
            let r = mark.rect;
            let (target, view) = if let Some((index, view)) = mark.destination {
                let target = *page_ids.get(index).ok_or("Invalid PDF link page")?;
                if !view.iter().all(|v| v.is_finite()) {
                    return Err("Invalid PDF link coordinates".into());
                }
                (target, view)
            } else {
                (page_id, r)
            };
            objects.push(format!("<< /Type /Annot /Subtype /Link /Rect [{:.3} {:.3} {:.3} {:.3}] /Contents {} /Border [0 0 0] /Dest [{target} 0 R /FitR {:.3} {:.3} {:.3} {:.3}] >>",r[0]-2.,r[1]-2.,r[2]+2.,r[3]+2.,unicode(&mark.details),view[0]-10.,view[1]-10.,view[2]+10.,view[3]+10.).into_bytes());
        }
        for (i, mark) in page.bookmarks.iter().enumerate() {
            let r = mark.rect;
            objects[child_ids[i]-1]=format!("<< /Title {} /Parent {outline_id} 0 R {} {} /Dest [{page_id} 0 R /FitR {:.3} {:.3} {:.3} {:.3}] >>",unicode(&mark.title),if i>0{format!("/Prev {} 0 R",child_ids[i-1])}else{String::new()},if i+1<child_ids.len(){format!("/Next {} 0 R",child_ids[i+1])}else{String::new()},r[0]-10.,r[1]-10.,r[2]+10.,r[3]+10.).into_bytes();
        }
        let children = if child_ids.is_empty() {
            String::new()
        } else {
            format!(
                "/First {} 0 R /Last {} 0 R /Count {}",
                child_ids[0],
                child_ids.last().unwrap(),
                child_ids.len()
            )
        };
        objects[outline_id - 1] = format!(
            "<< /Title {} /Parent 3 0 R {children} /Dest [{page_id} 0 R /Fit] >>",
            unicode(&page.title)
        )
        .into_bytes();
        let annotations = annotation_ids
            .iter()
            .map(|id| format!("{id} 0 R"))
            .collect::<Vec<_>>()
            .join(" ");
        objects[page_id-1]=format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.5} {:.5}] /Resources << /XObject << /Im0 {image_id} 0 R >> >> /Contents {content_id} 0 R /Annots [{annotations}] >>",page.width_pt,page.height_pt).into_bytes();
    }
    for (i, id) in outline_ids.iter().enumerate() {
        let mut object = String::from_utf8(objects[*id - 1].clone()).unwrap();
        let insert = format!(
            " {} {} ",
            if i > 0 {
                format!("/Prev {} 0 R", outline_ids[i - 1])
            } else {
                String::new()
            },
            if i + 1 < outline_ids.len() {
                format!("/Next {} 0 R", outline_ids[i + 1])
            } else {
                String::new()
            }
        );
        object.insert_str(object.len() - 2, &insert);
        objects[*id - 1] = object.into_bytes();
    }
    objects[0] =
        b"<< /Type /Catalog /Pages 2 0 R /Outlines 3 0 R /PageMode /UseOutlines >>".to_vec();
    objects[1] = format!(
        "<< /Type /Pages /Count {} /Kids [{}] >>",
        pages.len(),
        page_ids
            .iter()
            .map(|id| format!("{id} 0 R"))
            .collect::<Vec<_>>()
            .join(" ")
    )
    .into_bytes();
    objects[2] = format!(
        "<< /Type /Outlines /First {} 0 R /Last {} 0 R /Count {} >>",
        outline_ids[0],
        outline_ids.last().unwrap(),
        pages.len()
    )
    .into_bytes();
    let mut out = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend(object);
        out.extend(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for offset in offsets {
        out.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    Ok(out)
}
pub fn save(path: &Path, pages: &[Page]) -> Result<(), String> {
    crate::project_files::write_atomic(path, &serialize(pages)?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn page_validation_precedes_destination_replacement() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("test.pdf");
        std::fs::write(&path, b"original").unwrap();
        let page = Page {
            title: "Схема".into(),
            width_pt: 100.,
            height_pt: 100.,
            width: 2,
            height: 2,
            rgb: vec![255; 3],
            bookmarks: vec![],
        };
        assert!(save(&path, &[page]).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
    }
    #[test]
    fn invalid_cross_page_link_does_not_replace_destination() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("test.pdf");
        std::fs::write(&path, b"original").unwrap();
        let page = Page {
            title: "Sheet".into(),
            width_pt: 100.,
            height_pt: 100.,
            width: 2,
            height: 2,
            rgb: vec![255; 12],
            bookmarks: vec![Bookmark {
                title: "VCC".into(),
                rect: [10., 10., 20., 20.],
                details: "R1.1".into(),
                destination: Some((1, [10., 10., 20., 20.])),
            }],
        };
        assert!(save(&path, &[page]).unwrap_err().contains("link page"));
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
    }
}
