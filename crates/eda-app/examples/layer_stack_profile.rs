//! Read-only inspection of imported fabrication data.
fn main() -> Result<(), String> {
    let path = std::env::args_os().nth(1).ok_or("Pass a PCB document")?;
    let document = eda_io::load(std::path::Path::new(&path))?;
    let board = document.pcb.ok_or("Not a PCB")?;
    for (key, value) in &board.properties {
        if key.starts_with("V9_STACK_LAYER") {
            println!("{key}={value}");
        }
    }
    let stack = eda_core::layer_stack::Stack::from_board(&board);
    println!(
        "{}",
        serde_json::to_string_pretty(&stack).map_err(|e| e.to_string())?
    );
    stack.validate()
}
