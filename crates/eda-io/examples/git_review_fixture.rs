//! Reproducible synthetic Git repository for native-window review QA.
//! Refuses an existing destination; never reads or modifies a user design.
use std::{path::Path, process::Command};
fn git(path: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(())
}
fn main() -> Result<(), String> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Pass a NEW output directory")?;
    let path = Path::new(&path);
    std::fs::create_dir(path).map_err(|e| e.to_string())?;
    let write =
        |name: &str, data: &[u8]| std::fs::write(path.join(name), data).map_err(|e| e.to_string());
    git(path, &["init", "-b", "main"])?;
    git(path, &["config", "user.name", "PCBCraft QA"])?;
    git(path, &["config", "user.email", "qa@example.invalid"])?;
    write("Review.SchDoc", &schematic(false))?;
    let board = "|RECORD=Board|NAME=Git review|VX0=0mm|VY0=0mm|VX1=80mm|VY1=0mm|VX2=80mm|VY2=50mm|VX3=0mm|VY3=50mm\n|RECORD=Track|UNIQUEID=MOVE|LAYER=TOP|X1=10mm|Y1=10mm|X2=25mm|Y2=10mm|WIDTH=0.7mm\n|RECORD=Pad|UNIQUEID=PARAM|LAYER=TOP|NAME=1|X=40mm|Y=20mm|XSIZE=3mm|YSIZE=3mm|HOLESIZE=0mm|SHAPE=ROUND\n|RECORD=Via|UNIQUEID=REMOVE|X=60mm|Y=10mm|DIAMETER=3mm|HOLESIZE=1mm|STARTLAYER=TOP|ENDLAYER=BOTTOM\n|RECORD=Track|UNIQUEID=UNCHANGED|LAYER=TOP|X1=10mm|Y1=35mm|X2=65mm|Y2=35mm|WIDTH=0.4mm\n";
    write("Review.PcbDoc", board.as_bytes())?;
    write("Review.PrjPcb",b"[Design]\nVersion=1.0\n[Document1]\nDocumentPath=Review.SchDoc\n[Document2]\nDocumentPath=Review.PcbDoc\n")?;
    git(path, &["add", "."])?;
    git(path, &["commit", "-m", "Initial schematic and board"])?;
    write("Review.SchDoc", &schematic(true))?;
    let second = board
        .replace(
            "X1=10mm|Y1=10mm|X2=25mm|Y2=10mm",
            "X1=10mm|Y1=20mm|X2=25mm|Y2=20mm",
        )
        .replace("XSIZE=3mm|YSIZE=3mm", "XSIZE=5mm|YSIZE=3mm");
    let second = second
        .lines()
        .filter(|l| !l.contains("UNIQUEID=REMOVE"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n|RECORD=Fill|UNIQUEID=ADD|LAYER=TOP|X1=55mm|Y1=20mm|X2=65mm|Y2=25mm|ROTATION=0\n";
    write("Review.PcbDoc", second.as_bytes())?;
    git(path, &["add", "."])?;
    git(
        path,
        &[
            "commit",
            "-m",
            "Move, change parameters, remove and add geometry",
        ],
    )?;
    // Leave one reviewable saved edit for the Commit UI.
    write(
        "Review.PcbDoc",
        second.replace("WIDTH=0.4mm", "WIDTH=0.8mm").as_bytes(),
    )?;
    let remote = path.join("qa-remote.git");
    std::fs::create_dir(&remote).map_err(|e| e.to_string())?;
    git(&remote, &["init", "--bare"])?;
    git(
        path,
        &[
            "remote",
            "add",
            "origin",
            remote.to_str().ok_or("Invalid output path")?,
        ],
    )?;
    git(path, &["push", "-u", "origin", "main"])?;
    println!(
        "Synthetic Git QA project: {}",
        path.join("Review.PrjPcb").display()
    );
    Ok(())
}

fn schematic(changed: bool) -> Vec<u8> {
    let mut text = "|HEADER=Protel for Windows - Schematic Capture Ascii File Version 5.0|WEIGHT=5|MINORVERSION=9|UNIQUEID=PCBCraftGitReview\r\n|RECORD=31|FONTIDCOUNT=1|SIZE1=10|FONTNAME1=Arial|SYSTEMFONT=1|SHEETSTYLE=0|BORDERON=T|TITLEBLOCKON=F|AREACOLOR=16317695|VISIBLEGRIDON=F|VISIBLEGRIDSIZE=10|SNAPGRIDSIZE=10\r\n".to_owned();
    let rectangles = if changed {
        vec![
            ("MOVE", 150, 450, 1),
            ("PARAM", 350, 350, 3),
            ("UNCHANGED", 750, 350, 1),
            ("ADD", 550, 200, 1),
        ]
    } else {
        vec![
            ("MOVE", 150, 350, 1),
            ("PARAM", 350, 350, 1),
            ("REMOVE", 550, 350, 1),
            ("UNCHANGED", 750, 350, 1),
        ]
    };
    for (id, x, y, width) in rectangles {
        text.push_str(&format!("|RECORD=14|UNIQUEID={id}|LOCATION.X={x}|LOCATION.Y={y}|CORNER.X={}|CORNER.Y={}|COLOR=128|LINEWIDTH={width}\r\n",x+90,y+60));
    }
    text.into_bytes()
}
