//! Read-only connectivity diagnostic for local board validation.
fn main() {
    let path = std::env::args_os().nth(1).expect("board path");
    let document = eda_io::load(std::path::Path::new(&path)).expect("load board");
    let board = document.pcb.expect("PCB document");
    let start = std::time::Instant::now();
    let connections = eda_core::pcb_connectivity::connections(&board);
    let mut counts = std::collections::BTreeMap::new();
    for c in &connections {
        *counts.entry(c.net).or_insert(0usize) += 1;
    }
    for (net, count) in counts {
        println!("{}: {count}", board.net_name(Some(net)));
    }
    println!(
        "{} items, {} nets, {} unconnected island links, {:.3}s",
        board.items.len(),
        board.nets.len(),
        connections.len(),
        start.elapsed().as_secs_f64()
    );
}
