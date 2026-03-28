// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    eframe::run_native(
        "SoymilkyTracker",
        eframe::NativeOptions::default(),
        Box::new(|cc| Ok(Box::new(tracker_client::app::TrackerApp::new(cc)))),
    )
    .unwrap();
}
