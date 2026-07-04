// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if launched_as_cli() {
        if let Err(err) = i_am_working_lib::cli::run_from_env_args() {
            eprintln!("{err}");
            std::process::exit(1);
        }
        return;
    }

    i_am_working_lib::run()
}

fn launched_as_cli() -> bool {
    std::env::args_os()
        .next()
        .and_then(|path| {
            std::path::PathBuf::from(path)
                .file_name()
                .map(|name| name == "iaw")
        })
        .unwrap_or(false)
}
