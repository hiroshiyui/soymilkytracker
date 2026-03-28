// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

// Native entry point. Not compiled on WASM (wasm_main in lib.rs is used instead).
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "SoymilkyTracker",
        native_options,
        Box::new(|cc| Ok(Box::new(tracker_client::app::TrackerApp::new(cc)))),
    )
}
