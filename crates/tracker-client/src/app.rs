// SPDX-FileCopyrightText: 2026 HUIHONG YOU
// SPDX-License-Identifier: GPL-3.0-or-later

//! Main application struct. All UI code lives here.

/// IBM EGA 8×8 bitmap font (Ac437 variant, CC BY 4.0, VileR / int10h.org).
/// Used as the primary UI typeface for the classic DOS tracker aesthetic.
const IBM_EGA_8X8: &[u8] = include_bytes!("../../../assets/fonts/Ac437_IBM_EGA_8x8.ttf");

/// Name under which the tracker font is registered in egui's font system.
pub const FONT_TRACKER: &str = "tracker";

/// Install the IBM EGA 8×8 font into an egui context.
///
/// Call this once during app setup via [`eframe::CreationContext`].
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        FONT_TRACKER.to_owned(),
        egui::FontData::from_static(IBM_EGA_8X8).into(),
    );

    // Make the tracker font the first choice for both Proportional and Monospace
    // families so it is used by default throughout the UI.
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, FONT_TRACKER.to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, FONT_TRACKER.to_owned());

    ctx.set_fonts(fonts);
}

pub struct TrackerApp {}

impl TrackerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        Self {}
    }
}

impl eframe::App for TrackerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("SoymilkyTracker");
            ui.label("Work in progress.");
        });
    }
}
