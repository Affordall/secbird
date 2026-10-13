//! Secretarybird tray app: re-applies each device's profile when it is plugged in or the
//! PC wakes, and switches profiles from the tray menu. Plain Win32, no GUI toolkit.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod engine;
#[cfg(windows)]
mod server;
#[cfg(windows)]
mod win;

#[cfg(windows)]
fn main() {
    win::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("secbird-tray only runs on Windows");
}
