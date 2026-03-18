#![allow(dead_code)]

use eframe::egui::{Color32, RichText, Ui};

/// Returns a color appropriate for the given content type and theme.
pub fn type_color(item_type: &str, dark: bool) -> Color32 {
    match item_type {
        "Channel" => if dark {
            Color32::from_rgb(100, 200, 255)   // bright sky blue
        } else {
            Color32::from_rgb(0, 90, 160)      // deep navy
        },
        "Movie" => if dark {
            Color32::from_rgb(120, 230, 120)   // bright green
        } else {
            Color32::from_rgb(0, 120, 50)      // forest green
        },
        "Series" | "SeriesEpisode" => if dark {
            Color32::from_rgb(255, 215, 60)    // warm gold
        } else {
            Color32::from_rgb(140, 90, 0)      // amber
        },
        "error" => Color32::from_rgb(210, 50, 50),
        "warning" => if dark {
            Color32::from_rgb(255, 180, 40)
        } else {
            Color32::from_rgb(160, 90, 0)
        },
        "info" => if dark {
            Color32::from_rgb(100, 170, 255)
        } else {
            Color32::from_rgb(30, 90, 180)
        },
        "success" => if dark {
            Color32::from_rgb(120, 230, 120)
        } else {
            Color32::from_rgb(0, 120, 50)
        },
        _ => if dark { Color32::WHITE } else { Color32::BLACK },
    }
}

/// Create a colored text widget based on content type (dark theme).
/// Kept for backwards compatibility; prefer `colored_text_themed`.
pub fn colored_text_by_type(text: &str, item_type: &str) -> RichText {
    colored_text_themed(text, item_type, true)
}

/// Create a colored text widget based on content type and current theme.
pub fn colored_text_themed(text: &str, item_type: &str, dark: bool) -> RichText {
    RichText::new(text).color(type_color(item_type, dark))
}

/// Returns accent colors for UI chrome (title, highlights) per theme.
pub fn accent_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(100, 175, 255)   // bright blue
    } else {
        Color32::from_rgb(25, 80, 170)     // royal blue
    }
}

/// Star rating color per theme.
pub fn rating_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(255, 210, 60)
    } else {
        Color32::from_rgb(160, 110, 0)
    }
}

/// Play button fill color per theme.
pub fn play_button_fill(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(38, 105, 50)
    } else {
        Color32::from_rgb(25, 130, 60)
    }
}

/// Favorite star active color per theme.
pub fn fav_active_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(255, 200, 50)
    } else {
        Color32::from_rgb(180, 120, 0)
    }
}

/// Create a loading spinner
pub fn render_loading_spinner(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(text);
    });
}
