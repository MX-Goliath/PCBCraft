//! Read-only cross-probe audit of a local PrjPcb; does not copy source files.
use eda_core::cross_probe::{destinations, source};
fn main() -> Result<(), String> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("Usage: cross_probe_check <project.PrjPcb>")?;
    let project = eda_io::altium::load_project(std::path::Path::new(&path))?;
    let pcb_path = &project
        .documents
        .iter()
        .find(|d| d.kind == "pcbdoc")
        .ok_or("Project has no PCB")?
        .path;
    let pcb = eda_io::altium_pcb::load_pcb(pcb_path)?;
    let (mut total, mut linked, mut repeated) = (0, 0, 0);
    for entry in project
        .documents
        .iter()
        .filter(|d| d.kind == "schdoc" && d.exists)
    {
        let sch = eda_io::altium::load_schematic(&entry.path)?;
        for c in &sch
            .schematic
            .as_ref()
            .ok_or("No schematic scene")?
            .components
        {
            total += 1;
            let targets = destinations(&source(&sch, c.id)?, &pcb)?;
            linked += usize::from(!targets.is_empty());
            repeated += usize::from(targets.len() > 1);
        }
    }
    println!(
        "{linked}/{total} schematic components linked; {repeated} require physical-instance selection"
    );
    Ok(())
}
