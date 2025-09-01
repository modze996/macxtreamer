use std::fs;
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::mpsc;
use eframe::egui;
use reqwest;
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "MacXtreamer",
        options,
        Box::new(|_cc| Box::new(MacXtreamerApp::default())),
    )
}

#[derive(Debug)]
struct MacXtreamerApp {
    config: HashMap<String, String>,
    playlists: Vec<String>,
    vods: Vec<String>,
    series: Vec<String>,
    content_table: Vec<HashMap<String, String>>, 
    // Empfängt Playlists von einer Hintergrundaufgabe nach App-Start
    playlists_rx: Option<mpsc::Receiver<Result<Vec<String>, String>>>,
    // UI-Status
    is_loading: bool,
    last_error: Option<String>,
}

impl Default for MacXtreamerApp {
    fn default() -> Self {
        let config = Self::read_config("xtream_config.txt").unwrap_or_default();

        // Channel zum einmaligen Befüllen der Playlists per Hintergrundtask
        let (tx, rx) = mpsc::channel();
        let config_clone = config.clone();
        tokio::spawn(async move {
            let result = fetch_xtream_playlists_from(&config_clone)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });

        Self {
            config,
            playlists: vec![],
            vods: vec![],
            series: vec![],
            content_table: vec![],
            playlists_rx: Some(rx),
            is_loading: true,
            last_error: None,
        }
    }
}

impl MacXtreamerApp {
    // Funktion zum Lesen der Konfigurationsdatei
    fn read_config(file_path: &str) -> Result<HashMap<String, String>, io::Error> {
        let content = fs::read_to_string(file_path)?;
        let mut config = HashMap::new();
        for line in content.lines() {
            if let Some((key, value)) = line.split_once('=') {
                config.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
        Ok(config)
    }

    // Funktion zum Speichern der Konfigurationsdatei
    fn save_config(&self, file_path: &str) -> Result<(), io::Error> {
        let mut file = fs::File::create(file_path)?;
        for (key, value) in &self.config {
            writeln!(file, "{}={}", key, value)?;
        }
        Ok(())
    }

    // Funktion zum Laden von Kategorien (Playlists, VOD, Serien)
    fn load_categories(&mut self) {
        // Beispiel-Daten (kann durch API-Aufrufe ersetzt werden)
        self.playlists = vec!["News", "Sports", "Music"].into_iter().map(String::from).collect();
        self.vods = vec!["Movies", "Documentaries"].into_iter().map(String::from).collect();
        self.series = vec!["Drama", "Comedy"].into_iter().map(String::from).collect();
    }

    // Funktion zum Generieren von Stream-URLs
    fn build_stream_url(&self, stream_id: &str) -> String {
        format!(
            "{}/live/{}/{}/{}.ts",
            self.config.get("address").unwrap_or(&"http://example.com".to_string()),
            self.config.get("username").unwrap_or(&"user".to_string()),
            self.config.get("password").unwrap_or(&"pass".to_string()),
            stream_id
        )
    }

    fn build_vod_stream_url(&self, stream_id: &str, container_extension: Option<&str>) -> String {
        let ext = container_extension.unwrap_or("mp4");
        format!(
            "{}/movie/{}/{}/{}.{}",
            self.config.get("address").unwrap_or(&"http://example.com".to_string()),
            self.config.get("username").unwrap_or(&"user".to_string()),
            self.config.get("password").unwrap_or(&"pass".to_string()),
            stream_id,
            ext
        )
    }

    fn build_series_episode_stream_url(&self, episode_id: &str, container_extension: Option<&str>) -> String {
        let ext = container_extension.unwrap_or("mp4");
        format!(
            "{}/series/{}/{}/{}.{}",
            self.config.get("address").unwrap_or(&"http://example.com".to_string()),
            self.config.get("username").unwrap_or(&"user".to_string()),
            self.config.get("password").unwrap_or(&"pass".to_string()),
            episode_id,
            ext
        )
    }

    // Funktion zum Aktualisieren des Content-Tables
    fn update_content_table(&mut self, category: &str, name: &str) {
        self.content_table.clear();
        self.content_table.push(HashMap::from([
            ("Category".to_string(), category.to_string()),
            ("Name".to_string(), name.to_string()),
            ("Detail".to_string(), "Sample detail about the content".to_string()),
        ]));
    }

    fn load_content(&mut self, category: &str, name: &str) {
        // Simulate loading content based on category and name
        self.content_table.clear();
        self.content_table.push(HashMap::from([
            ("Category".to_string(), category.to_string()),
            ("Name".to_string(), name.to_string()),
            ("Detail".to_string(), "Sample detail about the content".to_string()),
        ]));
    }

    // Funktion zum Verwalten von Favoriten
    fn add_to_favorites(&mut self, item: HashMap<String, String>) {
        if !self.is_favorite(&item) {
            self.content_table.push(item);
        }
    }

    fn remove_favorite(&mut self, item_id: &str) {
        self.content_table.retain(|item| item.get("id") != Some(&item_id.to_string()));
    }

    fn is_favorite(&self, item: &HashMap<String, String>) -> bool {
        self.content_table.iter().any(|fav| fav == item)
    }

    // Funktion zum Verwalten von zuletzt abgespielten Inhalten
    fn add_to_recently_played(&mut self, item: HashMap<String, String>) {
        self.content_table.push(item);
        if self.content_table.len() > 10 {
            self.content_table.remove(0); // Älteste Einträge entfernen
        }
    }

    fn clear_recently_played(&mut self) {
        self.content_table.clear();
    }

    fn update_recently_played_list(&self) {
        for item in &self.content_table {
            println!("Recently Played: {:?}", item);
        }
    }

    fn update_favorites_list(&self) {
        for item in &self.content_table {
            println!("Favorite: {:?}", item);
        }
    }

    // Funktion zum Abrufen von Playlists von der Xtream-URL
    #[allow(dead_code)]
    async fn fetch_xtream_playlists(&mut self) {
        let base_url = self.config.get("address").unwrap_or(&"http://example.com".to_string()).clone();
        let username = self.config.get("username").unwrap_or(&"user".to_string()).clone();
        let password = self.config.get("password").unwrap_or(&"pass".to_string()).clone();

        let url = format!("{}/player_api.php?username={}&password={}&action=get_live_categories", base_url, username, password);

        match reqwest::get(&url).await {
            Ok(response) => {
                if let Ok(json) = response.json::<Value>().await {
                    if let Some(categories) = json.as_array() {
                        self.playlists = categories
                            .iter()
                            .filter_map(|cat| cat.get("category_name").and_then(|name| name.as_str()).map(String::from))
                            .collect();
                    }
                }
            }
            Err(err) => {
                eprintln!("Fehler beim Abrufen der Playlists: {}", err);
            }
        }
    }
}

impl eframe::App for MacXtreamerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Ergebnisse der Hintergrundaufgabe übernehmen (falls vorhanden)
        if let Some(rx) = &self.playlists_rx {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(list) => {
                        self.playlists = list;
                        self.last_error = None;
                    }
                    Err(err) => {
                        self.last_error = Some(err);
                    }
                }
                self.is_loading = false;
                self.playlists_rx = None; // einmalig
                ctx.request_repaint();
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("MacXtreamer");

            ui.horizontal(|ui| {
                if ui.button("Reload Playlists").clicked() && !self.is_loading {
                    // neuen Ladevorgang starten
                    let (tx, rx) = mpsc::channel();
                    let config_clone = self.config.clone();
                    self.is_loading = true;
                    self.last_error = None;
                    self.playlists_rx = Some(rx);
                    tokio::spawn(async move {
                        let result = fetch_xtream_playlists_from(&config_clone)
                            .await
                            .map_err(|e| e.to_string());
                        let _ = tx.send(result);
                    });
                }

                if self.is_loading {
                    ui.label("Loading playlists…");
                }
                if let Some(err) = &self.last_error {
                    ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
                }
            });

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label("Playlists");
                    let playlists = self.playlists.clone();
                    for playlist in playlists {
                        if ui.button(&playlist).clicked() {
                            self.update_content_table("playlist", &playlist);
                        }
                    }
                });

                ui.vertical(|ui| {
                    ui.label("VOD");
                    let vods = self.vods.clone();
                    for vod in vods {
                        if ui.button(&vod).clicked() {
                            self.update_content_table("vod", &vod);
                        }
                    }
                });

                ui.vertical(|ui| {
                    ui.label("Series");
                    let series = self.series.clone();
                    for series_item in series {
                        if ui.button(&series_item).clicked() {
                            self.update_content_table("series", &series_item);
                        }
                    }
                });
            });

            ui.separator();

            ui.label("Content Table");
            egui::Grid::new("content_table").show(ui, |ui| {
                for row in &self.content_table {
                    for (key, value) in row {
                        ui.label(format!("{}: {}", key, value));
                    }
                    ui.end_row();
                }
            });

            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("Add to Favorites").clicked() {
                    if let Some(selected) = self.content_table.first().cloned() {
                        self.add_to_favorites(selected);
                    }
                }

                if ui.button("Clear Recently Played").clicked() {
                    self.clear_recently_played();
                }
            });

            ui.separator();

            ui.label("Recently Played");
            for item in &self.content_table {
                ui.label(format!("Recently Played: {:?}", item));
            }

            ui.separator();

            ui.label("Favorites");
            for item in &self.content_table {
                ui.label(format!("Favorite: {:?}", item));
            }
        });
    }
}

// Hilfsfunktion: lädt Playlists anhand der Konfiguration (ohne Selbst-Referenz)
async fn fetch_xtream_playlists_from(config: &HashMap<String, String>) -> Result<Vec<String>, reqwest::Error> {
    let base_url = config
        .get("address")
        .unwrap_or(&"http://example.com".to_string())
        .clone();
    let username = config.get("username").unwrap_or(&"user".to_string()).clone();
    let password = config.get("password").unwrap_or(&"pass".to_string()).clone();

    let url = format!(
        "{}/player_api.php?username={}&password={}&action=get_live_categories",
        base_url, username, password
    );

    let response = reqwest::get(&url).await?;
    let json = response.json::<Value>().await?;
    let playlists = json
        .as_array()
        .map(|categories| {
            categories
                .iter()
                .filter_map(|cat| cat.get("category_name").and_then(|name| name.as_str()).map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(playlists)
}
