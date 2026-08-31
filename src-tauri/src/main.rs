// Windows release builds must not spawn a console window behind the HUD.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    dev_layer_lib::run()
}
