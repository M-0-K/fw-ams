//! fw-ams CLI – debugging and hardware calibration tool.

mod spike;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("spike") => spike::run(),
        _ => {
            eprintln!("usage: fw-ams <spike>");
            std::process::exit(2);
        }
    }
}
