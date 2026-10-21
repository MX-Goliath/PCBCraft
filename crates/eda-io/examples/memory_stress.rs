//! Local allocation/RSS probe; never saves or changes the input document.
use eda_core::{
    Document, History,
    schematic_edit::{self, Edit, Request},
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering},
};

struct TrackedAllocator;
static LIVE: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for TrackedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            LIVE.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) };
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let next = unsafe { System.realloc(ptr, layout, size) };
        if !next.is_null() {
            LIVE.fetch_add(size, Ordering::Relaxed);
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        next
    }
}
#[global_allocator]
static ALLOCATOR: TrackedAllocator = TrackedAllocator;

fn sample(mode: &str, iteration: usize, history: Option<&History>) {
    let live = LIVE.load(Ordering::Relaxed);
    let rss = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|n| n.parse::<usize>().ok())
                .map(|n| n * 1024)
        });
    println!(
        "{}",
        serde_json::json!({"mode":mode,"iteration":iteration,"live_allocated_bytes":live,"rss_bytes":rss,"history_steps":history.map(History::retained_steps),"history_estimated_bytes":history.map(History::estimated_retained_bytes)})
    );
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("Usage: memory_stress INPUT.SchDoc [ITERATIONS] [visibility|text|font|insert-delete|undo-redo|noop|preview|open-close]")?;
    let iterations: usize = args
        .next()
        .as_deref()
        .unwrap_or("400")
        .parse()
        .map_err(|_| "Invalid iteration count")?;
    let mode = args.next().unwrap_or_else(|| "visibility".into());
    if ![
        "visibility",
        "text",
        "font",
        "insert-delete",
        "undo-redo",
        "noop",
        "preview",
        "open-close",
    ]
    .contains(&mode.as_str())
    {
        return Err("Unknown probe mode".into());
    }
    let load = || -> Result<Document, String> {
        let path = std::path::Path::new(&path);
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("SchDoc"))
        {
            eda_io::altium::load_schematic(path)
        } else {
            eda_io::load(path)
        }
    };
    sample("before-load", 0, None);
    {
        let mut doc = load()?;
        let id = doc
            .schematic
            .as_ref()
            .ok_or("Not a schematic")?
            .primitives
            .iter()
            .find(|p| matches!(p.kind, eda_core::schematic::SchPrimitiveKind::Text { .. }))
            .ok_or("No schematic text")?
            .id;
        let mut history = History::default();
        sample(&mode, 0, Some(&history));
        for iteration in 1..=iterations {
            if mode == "open-close" {
                std::hint::black_box(load()?);
            } else if mode == "undo-redo" && iteration > 1 {
                history.undo(&mut doc);
                history.redo(&mut doc);
            } else if mode == "font" {
                let mut font = doc.schematic.as_ref().unwrap().fonts[0].clone();
                font.height_nm = 1_000_000 + (iteration % 1000) as i64 * 10_000;
                let request = Request {
                    document: doc.id,
                    expected_revision: history.revision,
                    edit: Edit::Font { id, font },
                };
                history.schematic_edit(&mut doc, request)?;
            } else if mode == "insert-delete" {
                let p = schematic_edit::primitive(
                    eda_core::schematic::SchPrimitiveKind::Text {
                        position: eda_geom::Point::mm(1, 1),
                        text: "Probe".into(),
                        font: 0,
                        orientation: 0,
                        justification: 0,
                    },
                    None,
                );
                let inserted = p.id;
                for edit in [
                    Edit::Insert(schematic_edit::Fragment {
                        fonts: doc.schematic.as_ref().unwrap().fonts.clone(),
                        primitives: vec![p],
                        ..Default::default()
                    }),
                    Edit::Delete(inserted),
                ] {
                    let request = Request {
                        document: doc.id,
                        expected_revision: history.revision,
                        edit,
                    };
                    history.schematic_edit(&mut doc, request)?;
                }
            } else {
                let mut value = doc
                    .schematic
                    .as_ref()
                    .unwrap()
                    .primitive(id)
                    .unwrap()
                    .clone();
                if mode == "text" {
                    if let eda_core::schematic::SchPrimitiveKind::Text { text, .. } =
                        &mut value.kind
                    {
                        *text = format!("Probe value {iteration}");
                    }
                    if value.presentation.raw_text.is_some() {
                        value.presentation.raw_text = Some(format!("Probe value {iteration}"));
                    }
                } else if mode != "noop" {
                    value.presentation.hidden = !value.presentation.hidden;
                }
                let edit = Edit::PrimitiveProperties {
                    id,
                    value: Box::new(value),
                };
                if mode == "preview" {
                    std::hint::black_box(schematic_edit::preview(&doc, &edit)?);
                } else {
                    let request = Request {
                        document: doc.id,
                        expected_revision: history.revision,
                        edit,
                    };
                    history.schematic_edit(&mut doc, request)?;
                }
            }
            if iteration % 50 == 0 || iteration == iterations {
                sample(&mode, iteration, Some(&history));
            }
        }
        println!(
            "{}",
            serde_json::json!({"mode":mode,"font_definitions":doc.schematic.as_ref().unwrap().fonts.len()})
        );
    }
    sample("after-close", iterations, None);
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
