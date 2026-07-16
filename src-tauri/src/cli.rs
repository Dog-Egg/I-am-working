//! Stable public facade for the `iaw` binary.

pub fn run_from_env_args() -> Result<(), String> {
    crate::features::nosleep::cli::run_from_env_args()
}

pub fn run_from_args(args: impl IntoIterator<Item = String>) -> Result<(), String> {
    crate::features::nosleep::cli::run_from_args(args)
}
