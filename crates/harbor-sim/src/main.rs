fn main() {
    if let Err(e) = harbor_app::run() {
        eprintln!("harbor-sim: {e}");
        std::process::exit(1);
    }
}
