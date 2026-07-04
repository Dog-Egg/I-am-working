fn main() {
    if let Err(err) = i_am_working_lib::cli::run_from_env_args() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
