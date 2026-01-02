use crate::models::{Item, SearchItem};
use rayon::prelude::*;

/// Score a candidate string against the query.
/// Higher is better. Only substring matches are allowed - no fuzzy matching.
fn score_candidate(candidate: &str, query: &str) -> f64 {
    if query.is_empty() { return 0.0; }
    let c = candidate.to_lowercase();
    let q = query.to_lowercase();
    
    // Exact match - highest score
    if c == q { return 100.0; }
    
    // Starts with query - very high score  
    if c.starts_with(&q) { return 95.0; }
    
    // Contains query as substring - high score
    if c.contains(&q) { return 85.0; }
    
    // No fuzzy matching - if query is not a substring, return 0
    0.0
}

/// Aggregate best score across name and plot for an item.
fn score_item(item: &Item, query: &str) -> f64 {
    let name_score = score_candidate(&item.name, query);
    
    // Only consider plot if name doesn't match well enough
    // This prevents items with query in plot but not in name from ranking too high
    if name_score >= 85.0 {
        return name_score; // Good name match, ignore plot
    }
    
    let plot_score = if item.plot.is_empty() { 
        0.0 
    } else { 
        score_candidate(&item.plot, query) * 0.4 // plot significantly less weighted
    };
    
    name_score.max(plot_score)
}

/// Search with language filter support
pub fn search_items_with_language_filter(
    movies: &Vec<Item>, 
    series: &Vec<Item>, 
    channels: &Vec<Item>, 
    text: &str,
    language_filter: &[String]
) -> Vec<SearchItem> {
    let query = text.trim();
    if query.is_empty() { return Vec::new(); }
    
    // Für sehr kurze Queries (<=2 Zeichen) nur einfache substring Suche (Performance + Erwartung)
    if query.len() <= 2 { 
        return legacy_substring_with_filter(movies, series, channels, query, language_filter); 
    }
    
    // Use HashMap to deduplicate by ID and keep best score
    let mut best_scores: std::collections::HashMap<String, (f64, &Item, &'static str)> = std::collections::HashMap::new();
    
    // Parallelisiere das Scoring mit Rayon
    let movie_scores: Vec<(String, (f64, &Item, &'static str))> = movies.par_iter()
        .filter_map(|m| {
            let sc = score_item(m, query);
            if sc > 0.0 {
                Some((m.id.clone(), (sc, m, "Movie")))
            } else {
                None
            }
        })
        .collect();

    let series_scores: Vec<(String, (f64, &Item, &'static str))> = series.par_iter()
        .filter_map(|s| {
            let sc = score_item(s, query);
            if sc > 0.0 {
                Some((s.id.clone(), (sc, s, "Series")))
            } else {
                None
            }
        })
        .collect();

    let channel_scores: Vec<(String, (f64, &Item, &'static str))> = channels.par_iter()
        .filter_map(|c| {
            let sc = score_item(c, query);
            if sc > 0.0 {
                Some((c.id.clone(), (sc, c, "Channel")))
            } else {
                None
            }
        })
        .collect();

    // Kombiniere alle Scores und dedupliziere nach bester Bewertung
    for (id, (sc, item, kind)) in movie_scores.into_iter().chain(series_scores).chain(channel_scores) {
        best_scores.entry(id)
            .and_modify(|e| { if sc > e.0 { *e = (sc, item, kind); } })
            .or_insert((sc, item, kind));
    }
    
    if best_scores.is_empty() {
        // Fallback: alte substring Logik (kein Score Filter)
        return legacy_substring(movies, series, channels, query);
    }
    
    // Convert to vector and sort by score
    let mut scored: Vec<(f64, &Item, &'static str)> = best_scores.into_iter().map(|(_, v)| v).collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(500);
    
    // Convert to SearchItems, apply language filter
    let results: Vec<SearchItem> = scored.into_iter()
        .map(|(_sc, it, kind)| SearchItem {
            id: it.id.clone(),
            name: it.name.clone(),
            info: kind.into(),
            container_extension: it.container_extension.clone(),
            cover: it.cover.clone(),
            year: it.year.clone(),
            release_date: it.release_date.clone(),
            rating_5based: it.rating_5based,
            genre: it.genre.clone(),
        })
        .filter(|item| {
            if language_filter.is_empty() {
                true
            } else {
                // Check if item language matches any selected filter
                if let Some(lang) = crate::helpers::extract_language_from_name(&item.name) {
                    language_filter.contains(&lang)
                } else {
                    // Exclude items without detectable language when filter is active
                    false
                }
            }
        })
        .collect();
    
    results
}

fn legacy_substring(movies: &Vec<Item>, series: &Vec<Item>, channels: &Vec<Item>, q: &str) -> Vec<SearchItem> {
    legacy_substring_with_filter(movies, series, channels, q, &[])
}

fn legacy_substring_with_filter(movies: &Vec<Item>, series: &Vec<Item>, channels: &Vec<Item>, q: &str, language_filter: &[String]) -> Vec<SearchItem> {
    let t = q.to_lowercase();
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out = Vec::new();
    
    let matches_filter = |item: &Item| {
        if language_filter.is_empty() {
            true
        } else {
            if let Some(lang) = crate::helpers::extract_language_from_name(&item.name) {
                language_filter.contains(&lang)
            } else {
                false // Exclude items without detectable language when filter is active
            }
        }
    };
    
    for m in movies {
        if (m.name.to_lowercase().contains(&t) || m.plot.to_lowercase().contains(&t)) 
            && seen_ids.insert(m.id.clone()) && matches_filter(m) {
            out.push(SearchItem { 
                id: m.id.clone(), 
                name: m.name.clone(), 
                info: "Movie".into(), 
                container_extension: m.container_extension.clone(), 
                cover: m.cover.clone(), 
                year: m.year.clone(), 
                release_date: m.release_date.clone(), 
                rating_5based: m.rating_5based, 
                genre: m.genre.clone() 
            });
        }
    }
    for s in series {
        if (s.name.to_lowercase().contains(&t) || s.plot.to_lowercase().contains(&t))
            && seen_ids.insert(s.id.clone()) && matches_filter(s) {
            out.push(SearchItem { 
                id: s.id.clone(), 
                name: s.name.clone(), 
                info: "Series".into(), 
                container_extension: s.container_extension.clone(), 
                cover: s.cover.clone(), 
                year: s.year.clone(), 
                release_date: s.release_date.clone(), 
                rating_5based: s.rating_5based, 
                genre: s.genre.clone() 
            });
        }
    }
    for c in channels {
        if (c.name.to_lowercase().contains(&t) || c.plot.to_lowercase().contains(&t))
            && seen_ids.insert(c.id.clone()) && matches_filter(c) {
            out.push(SearchItem { 
                id: c.id.clone(), 
                name: c.name.clone(), 
                info: "Channel".into(), 
                container_extension: c.container_extension.clone(), 
                cover: c.cover.clone(), 
                year: c.year.clone(), 
                release_date: c.release_date.clone(), 
                rating_5based: c.rating_5based, 
                genre: c.genre.clone() 
            });
        }
    }
    out
}
