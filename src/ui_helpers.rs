#![allow(dead_code)]

use eframe::egui::{self, Color32, RichText, Ui};
use std::path::Path;

use crate::downloads::{DownloadState, DownloadStatus, format_bytes, format_speed, format_eta};

/// Render a colored status badge for downloads
pub fn render_download_status_badge(ui: &mut Ui, status: &DownloadStatus) {
    let (text, color) = match status {
        DownloadStatus::Queued => ("Queued", Color32::GRAY),
        DownloadStatus::Downloading => ("Downloading", Color32::BLUE),
        DownloadStatus::Paused => ("Paused", Color32::YELLOW),
        DownloadStatus::Completed => ("Completed", Color32::GREEN),
        DownloadStatus::Failed(_) => ("Failed", Color32::RED),
        DownloadStatus::Cancelled => ("Cancelled", Color32::DARK_RED),
    };
    
    ui.colored_label(color, text);
}

/// Render a progress bar for downloads
pub fn render_download_progress(ui: &mut Ui, download: &DownloadState) {
    let progress_bar = egui::ProgressBar::new(download.progress)
        .text(format!("{:.1}%", download.progress * 100.0));
    
    ui.add(progress_bar);
    
    // Show additional info
    ui.horizontal(|ui| {
        ui.label(format_bytes(download.downloaded_bytes));
        if let Some(total) = download.total_bytes {
            ui.label(format!("/ {}", format_bytes(total)));
        }
        if download.speed_bps > 0.0 {
            ui.label(format!("({}))", format_speed(download.speed_bps)));
        }
        if let Some(eta) = download.eta_seconds {
            ui.label(format!("ETA: {}", format_eta(Some(eta))));
        }
    });
}

/// Create a colored text widget based on content type
pub fn colored_text_by_type(text: &str, item_type: &str) -> RichText {
    let color = match item_type {
        "Channel" => Color32::LIGHT_BLUE,
        "Movie" => Color32::LIGHT_GREEN,
        "SeriesEpisode" => Color32::YELLOW,
        _ => Color32::WHITE,
    };
    RichText::new(text).color(color)
}

/// Render a collapsible section with a header
pub fn render_collapsible_section<R>(
    ui: &mut Ui,
    title: &str,
    id_source: &str,
    default_open: bool,
    content: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    egui::CollapsingHeader::new(title)
        .id_source(id_source)
        .default_open(default_open)
        .show(ui, content)
        .body_returned
}

/// Render a table header with sorting capabilities
pub fn render_sortable_header(
    ui: &mut Ui,
    title: &str,
    sort_key: &str,
    current_sort: &Option<String>,
    ascending: bool,
) -> bool {
    let is_current = current_sort.as_ref().map(|s| s.as_str()) == Some(sort_key);
    let mut clicked = false;
    
    ui.horizontal(|ui| {
        if ui.button(title).clicked() {
            clicked = true;
        }
        
        if is_current {
            let arrow = if ascending { "↑" } else { "↓" };
            ui.label(arrow);
        }
    });
    
    clicked
}

/// Create a search input field with clear button
pub fn render_search_field(
    ui: &mut Ui,
    search_text: &mut String,
    placeholder: &str,
) -> bool {
    let mut search_changed = false;
    
    ui.horizontal(|ui| {
        let response = ui.text_edit_singleline(search_text);
        if response.changed() {
            search_changed = true;
        }
        
        if search_text.is_empty() {
            response.widget_info(|| egui::WidgetInfo::text_edit(placeholder, placeholder));
        }
        
        if ui.button("Clear").clicked() && !search_text.is_empty() {
            search_text.clear();
            search_changed = true;
        }
    });
    
    search_changed
}

/// Format file size with appropriate units
pub fn format_file_size(size_bytes: Option<u64>) -> String {
    match size_bytes {
        Some(size) => format_bytes(size),
        None => "Unknown".to_string(),
    }
}

/// Create a tooltip for UI elements
pub fn add_tooltip(ui: &mut Ui, text: &str) -> egui::Response {
    ui.label("ⓘ").on_hover_text(text)
}

/// Render a status indicator (colored circle)
pub fn render_status_indicator(ui: &mut Ui, color: Color32, tooltip: &str) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(10.0, 10.0),
        egui::Sense::hover(),
    );
    ui.painter().circle_filled(rect.center(), 5.0, color);
    if ui.rect_contains_pointer(rect) {
        egui::show_tooltip_at_pointer(ui.ctx(), egui::Id::new("status_tooltip"), |ui| {
            ui.label(tooltip);
        });
    }
}

/// Create a responsive grid layout
pub fn create_responsive_grid(
    ui: &mut Ui,
    _items_count: usize,
    min_item_width: f32,
    max_columns: usize,
) -> egui_extras::TableBuilder<'_> {
    let available_width = ui.available_width();
    let columns = ((available_width / min_item_width) as usize).min(max_columns).max(1);
    
    egui_extras::TableBuilder::new(ui)
        .columns(egui_extras::Column::remainder(), columns)
}

/// Helper to create consistent button styling
pub fn styled_button(text: &str, color: Color32) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).color(color))
}

/// Helper to create warning/error messages
pub fn render_message(ui: &mut Ui, message: &str, message_type: MessageType) {
    let (color, icon) = match message_type {
        MessageType::Info => (Color32::LIGHT_BLUE, "ℹ"),
        MessageType::Warning => (Color32::YELLOW, "⚠"),
        MessageType::Error => (Color32::RED, "❌"),
        MessageType::Success => (Color32::GREEN, "✓"),
    };
    
    ui.horizontal(|ui| {
        ui.colored_label(color, icon);
        ui.colored_label(color, message);
    });
}

pub enum MessageType {
    Info,
    Warning,
    Error,
    Success,
}

/// Helper to format duration in a human-readable way
pub fn format_duration(seconds: Option<u32>) -> String {
    match seconds {
        Some(total_seconds) => {
            let hours = total_seconds / 3600;
            let minutes = (total_seconds % 3600) / 60;
            let seconds = total_seconds % 60;
            
            if hours > 0 {
                format!("{}:{:02}:{:02}", hours, minutes, seconds)
            } else {
                format!("{}:{:02}", minutes, seconds)
            }
        }
        None => "Unknown".to_string(),
    }
}

/// Helper to create consistent spacing
pub fn add_space(ui: &mut Ui, space: f32) {
    ui.add_space(space);
}

/// Helper to create a separator line
pub fn add_separator(ui: &mut Ui) {
    ui.separator();
}

/// Helper to create a vertical separator in horizontal layouts
pub fn add_vertical_separator(ui: &mut Ui) {
    ui.separator();
}

/// Helper to truncate text with ellipsis if too long
pub fn truncate_text(text: &str, max_length: usize) -> String {
    if text.len() <= max_length {
        text.to_string()
    } else {
        format!("{}...", &text[..max_length.saturating_sub(3)])
    }
}

/// Create a loading spinner
pub fn render_loading_spinner(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(text);
    });
}

/// Helper to create consistent margins around content
pub fn with_margin<R>(ui: &mut Ui, margin: f32, content: impl FnOnce(&mut Ui) -> R) -> R {
    ui.allocate_ui_with_layout(
        ui.available_size(),
        egui::Layout::top_down(egui::Align::LEFT).with_main_wrap(false),
        |ui| {
            ui.add_space(margin);
            content(ui)
        },
    ).inner
}

/// Convert a file path to a file:// URI
pub fn file_path_to_uri(p: &Path) -> String {
    // Simple percent-encode spaces only (sufficient for our filenames)
    let s = p.to_string_lossy().replace(' ', "%20");
    if s.starts_with('/') {
        format!("file://{}", s)
    } else if s.starts_with("file://") {
        s
    } else {
        format!("file://{}", s)
    }
}
