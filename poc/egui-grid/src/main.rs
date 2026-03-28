// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

mod app;

// `main` must be unconditionally present so the bin target compiles on WASM
// (Trunk compiles it even though it only uses the cdylib lib target).
// The native-only body is cfg-gated inside.
fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 600.0]),
            ..Default::default()
        };
        eframe::run_native(
            "PoC — egui Tracker Grid",
            options,
            Box::new(|cc| Ok(Box::new(app::TrackerGridApp::new(cc)))),
        )
        .unwrap();
    }
}
