#![allow(dead_code)]

use eframe::egui::{Color32, RichText, Ui};

/// Create a colored text widget based on content type
pub fn colored_text_by_type(text: &str, item_type: &str) -> RichText {
    let color = match item_type {
        "Channel" => Color32::LIGHT_BLUE,
        "Movie" => Color32::LIGHT_GREEN,
        "SeriesEpisode" => Color32::YELLOW,
        "error" => Color32::RED,
        "warning" => Color32::YELLOW,
        _ => Color32::WHITE,
    };
    RichText::new(text).color(color)
}

/// Create a loading spinner
pub fn render_loading_spinner(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(text);
    });
}
