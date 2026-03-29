// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    eframe::run_native(
        "SoymilkyTracker",
        eframe::NativeOptions {
            // Start maximised — fills the desktop without going fullscreen.
            viewport: egui::ViewportBuilder::default().with_maximized(true),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(tracker_client::app::TrackerApp::new(cc)))),
    )
    .unwrap();
}
