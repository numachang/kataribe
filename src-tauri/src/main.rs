// リリースビルドで余計なコンソールウィンドウを出さない
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    kataribe_app::run();
}
