use crate::models::Language;

/// Translation function - returns localized string based on language
pub fn t(key: &str, lang: Language) -> String {
    match (key, lang) {
        // AI Panel
        ("sidebar_title", Language::English) => "📌 Sidebar",
        ("sidebar_title", Language::German) => "📌 Seitenleiste",
        ("recommendations", Language::English) => "🧠 Recommendations",
        ("recommendations", Language::German) => "🧠 Empfehlungen",
        ("recently_added", Language::English) => "🆕 Recently Added",
        ("recently_added", Language::German) => "🆕 Zuletzt hinzugefügt",
        ("loading_content", Language::English) => "📭 Loading new content...",
        ("loading_content", Language::German) => "📭 Lade neue Inhalte...",
        ("loading_newest", Language::English) => "Loading newest VOD/Series...",
        ("loading_newest", Language::German) => "Die neuesten VOD/Serien werden geladen.",
        ("newly_added", Language::English) => "🆕 Newly Added",
        ("newly_added", Language::German) => "🆕 Neu hinzugefügt",
        
        // Settings
        ("settings", Language::English) => "⚙️ Settings",
        ("settings", Language::German) => "⚙️ Einstellungen",
        ("language", Language::English) => "Language",
        ("language", Language::German) => "Sprache",
        ("font_scale", Language::English) => "Font Scale",
        ("font_scale", Language::German) => "Schriftgröße",
        ("save", Language::English) => "💾 Save",
        ("save", Language::German) => "💾 Speichern",
        ("cancel", Language::English) => "❌ Cancel",
        ("cancel", Language::German) => "❌ Abbrechen",
        
        // Main UI
        ("live", Language::English) => "Live",
        ("live", Language::German) => "Live",
        ("vod", Language::English) => "VOD",
        ("vod", Language::German) => "VOD",
        ("series", Language::English) => "Series",
        ("series", Language::German) => "Serien",
        ("search", Language::English) => "🔍 Search",
        ("search", Language::German) => "🔍 Suche",
        ("favorites", Language::English) => "Favorites",
        ("favorites", Language::German) => "Favoriten",
        ("downloads", Language::English) => "Downloads",
        ("downloads", Language::German) => "Downloads",
        ("recently_played", Language::English) => "Recently played",
        ("recently_played", Language::German) => "Kürzlich abgespielt",
        
        // Downloads
        ("no_downloads", Language::English) => "📭 No downloads",
        ("no_downloads", Language::German) => "📭 Keine Downloads",
        ("enable_downloads_hint", Language::English) => "Enable downloads in settings to use this feature.",
        ("enable_downloads_hint", Language::German) => "Aktiviere Downloads in den Einstellungen um diese Funktion zu nutzen.",
        
        // Fallback
        _ => key,
    }.to_string()
}
