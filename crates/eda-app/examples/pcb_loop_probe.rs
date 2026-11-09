//! Read-only native-board loop replacement probe; does not save source files.
use eda_core::pcb::{Shape, TOP};
use eda_core::pcb_route::{Conflict, RouteSession};
use eda_geom::Point;

fn main() -> Result<(), String> {
    let path = std::env::args_os().nth(1).ok_or("Pass a PcbDoc")?;
    let doc = eda_io::load(std::path::Path::new(&path))?;
    let b = doc.pcb.as_ref().ok_or("Not a board")?;
    let name = std::env::args().nth(2).unwrap_or("AVCC_ADC".into());
    let net = b
        .nets
        .iter()
        .find(|n| n.name == name)
        .ok_or("Net not found")?;
    println!(
        "net={} index={} properties={:?}",
        net.name, net.index, net.properties
    );
    for (n, i) in b
        .items
        .iter()
        .enumerate()
        .filter(|(_, i)| i.net == Some(net.index))
    {
        println!(
            "index={n} layer={} locked={} component={:?} polygon={:?} shape={:?}",
            i.layer, i.locked, i.component, i.polygon, i.shape
        );
    }
    let mut cases = 0;
    let mut replaced = 0;
    for i in b.items.iter().filter(|i| {
        i.net == Some(net.index)
            && i.layer == TOP
            && i.component.is_none()
            && i.polygon.is_none()
            && !i.locked
    }) {
        let Shape::Track { start, end, .. } = i.shape else {
            continue;
        };
        if start == end {
            continue;
        }
        let a = Point {
            x: start.x + (end.x - start.x) / 4,
            y: start.y + (end.y - start.y) / 4,
        };
        let z = Point {
            x: start.x + 3 * (end.x - start.x) / 4,
            y: start.y + 3 * (end.y - start.y) / 4,
        };
        let mut r = RouteSession::begin(b, 0, a, TOP, None, 250_000, 0)?;
        r.conflict = Conflict::Ignore;
        r.place(
            Point {
                x: a.x,
                y: a.y + 2_000_000,
            },
            None,
        )?;
        r.place(
            Point {
                x: z.x,
                y: z.y + 2_000_000,
            },
            None,
        )?;
        r.place(z, None)?;
        let f = r.finish(0)?;
        cases += 1;
        replaced += usize::from(f.items.len() < r.draft().items.len());
        println!(
            "a={a:?} z={z:?} removed={}",
            r.draft().items.len() - f.items.len()
        );
    }
    println!("cases={cases} replaced={replaced}");
    Ok(())
}
