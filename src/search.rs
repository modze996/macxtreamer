use crate::models::{Item, SearchItem};

pub fn search_items(movies: &Vec<Item>, series: &Vec<Item>, text: &str) -> Vec<SearchItem> {
    let t = text.to_lowercase();
    let mut out = Vec::new();
    for m in movies {
        if m.name.to_lowercase().contains(&t) || m.plot.to_lowercase().contains(&t) {
            out.push(SearchItem { id: m.id.clone(), name: m.name.clone(), info: "Movie".into(), container_extension: m.container_extension.clone(), cover: m.cover.clone(), year: m.year.clone(), release_date: m.release_date.clone(), rating_5based: m.rating_5based, genre: m.genre.clone() });
        }
    }
    for s in series {
        if s.name.to_lowercase().contains(&t) || s.plot.to_lowercase().contains(&t) {
            out.push(SearchItem { id: s.id.clone(), name: s.name.clone(), info: "Series".into(), container_extension: s.container_extension.clone(), cover: s.cover.clone(), year: s.year.clone(), release_date: s.release_date.clone(), rating_5based: s.rating_5based, genre: s.genre.clone() });
        }
    }
    out
}
