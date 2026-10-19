// Hides the console window on Windows release builds; Android starts from lib.rs instead.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    shelf_lib::run()
}
