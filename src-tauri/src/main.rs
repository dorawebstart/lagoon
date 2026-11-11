// У релізній збірці не показувати чорне вікно консолі поруч із застосунком.
// `cfg_attr(умова, атрибут)` вмикає атрибут лише коли умова істинна:
// `not(debug_assertions)` = «це не debug-збірка».
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// main — точка входу будь-якої Rust-програми. Уся логіка — у lib.rs,
// тут лише виклик функції run() з нашої бібліотеки lagoon_lib.
fn main() {
    lagoon_lib::run()
}
