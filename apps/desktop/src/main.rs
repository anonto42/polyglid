fn main() {
    if let Err(error) = polyglid_desktop::launch() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
