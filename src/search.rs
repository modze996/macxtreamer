use crate::models::{Item, SearchItem};

/// Compute a simple Levenshtein distance (case-insensitive already handled outside)
fn levenshtein(a: &str, b: &str) -> usize {
    if a.is_empty() { return b.len(); }
    if b.is_empty() { return a.len(); }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len()+1];
    for (i, ca) in a.chars().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            cur[j+1] = (prev[j+1] + 1)
                .min(cur[j] + 1)
                .min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Score a candidate string against the query.
/// Higher is better. Substring matches get high base scores; distance adjusts otherwise.
fn score_candidate(candidate: &str, query: &str) -> f64 {
    if query.is_empty() { return 0.0; }
    let c = candidate.to_lowercase();
    let q = query.to_lowercase();
    if c == q { return 100.0; }
    if c.starts_with(&q) { return 95.0; }
    if c.contains(&q) { return 85.0; }
    // Fuzzy fallback: use Levenshtein normalized
    let dist = levenshtein(&c, &q) as f64;
    let len = c.len().max(q.len()) as f64;
    let similarity = 1.0 - (dist / len).min(1.0); // 0..1
    // Scale into 0..70 range (below strict substring matches)
    similarity * 70.0
}

/// Aggregate best score across name and plot for an item.
fn score_item(item: &Item, query: &str) -> f64 {
    let name_score = score_candidate(&item.name, query);
    let plot_score = if item.plot.is_empty() { 0.0 } else { score_candidate(&item.plot, query) * 0.6 }; // plot weniger gewichten
    name_score.max(plot_score)
}

/// Fuzzy + substring search across movies and series.
/// Returns sorted results (best score first) and filters out low quality matches.
pub fn search_items(movies: &Vec<Item>, series: &Vec<Item>, text: &str) -> Vec<SearchItem> {
    let query = text.trim();
    if query.is_empty() { return Vec::new(); }
    let mut scored: Vec<(f64, &Item, &'static str)> = Vec::new();
    for m in movies {
        let sc = score_item(m, query);
        if sc >= 35.0 { // Schwelle für Relevanz
            scored.push((sc, m, "Movie"));
        }
    }
    for s in series {
        let sc = score_item(s, query);
        if sc >= 35.0 {
            scored.push((sc, s, "Series"));
        }
    }
    // Sort descending by score
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    // Limit extreme result sets (performance safeguard)
    let max_results = 500; // arbitrary cap
    scored.truncate(max_results);
    scored.into_iter().map(|(_sc, it, kind)| SearchItem {
        id: it.id.clone(),
        name: it.name.clone(),
        info: kind.into(),
        container_extension: it.container_extension.clone(),
        cover: it.cover.clone(),
        year: it.year.clone(),
        release_date: it.release_date.clone(),
        rating_5based: it.rating_5based,
        genre: it.genre.clone(),
    }).collect()
}
