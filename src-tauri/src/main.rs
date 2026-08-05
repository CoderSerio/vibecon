// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(debug_assertions)]
    if std::env::args_os().any(|argument| argument == "--request-accessibility") {
        match vibecon_lib::request_pointer_accessibility() {
            Ok(message) => println!("VibeCon Accessibility request: {message}"),
            Err(error) => {
                eprintln!("VibeCon Accessibility request failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    #[cfg(debug_assertions)]
    if std::env::args_os().any(|argument| argument == "--pointer-self-test") {
        match vibecon_lib::run_pointer_self_test() {
            Ok(message) => println!("VibeCon pointer self-test: {message}"),
            Err(error) => {
                eprintln!("VibeCon pointer self-test failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    vibecon_lib::run()
}
