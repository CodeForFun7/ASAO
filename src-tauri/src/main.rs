// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // When started as the administrator helper for leftover cleanup, do that job and exit
    // without opening any window.
    if asao_lib::residual::elevated::maybe_run_from_args() {
        return;
    }
    asao_lib::run()
}
