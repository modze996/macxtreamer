use eframe::egui;
use crate::app_state::Msg;
use crate::models::{Config, Item, Language};
use crate::i18n::t;
use std::sync::mpsc::Sender;

/// Render the AI recommendations panel with tabs (Wisdom-Gate, Perplexity, Cognora, Gemini, OpenAI) and Recently Added
pub fn render_ai_panel(
    ui: &mut egui::Ui,
    config: &Config,
    wisdom_gate_recommendations: &Option<String>,
    recently_added_items: &[Item],
    ai_panel_tab: &mut String,
    tx: &Sender<Msg>,
) {
    let lang = config.language;
    ui.heading(t("sidebar_title", lang));
    ui.add_space(5.0);

    // Tab selection
    ui.horizontal(|ui| {
        if ui.selectable_label(*ai_panel_tab == "recommendations", t("recommendations", lang)).clicked() {
            *ai_panel_tab = "recommendations".to_string();
        }
        if ui.selectable_label(*ai_panel_tab == "recently_added", t("recently_added", lang)).clicked() {
            *ai_panel_tab = "recently_added".to_string();
        }
    });
    ui.separator();
    ui.add_space(5.0);

    match ai_panel_tab.as_str() {
        "recently_added" => {
            // Render recently added items
            render_recently_added_tab(ui, recently_added_items, lang, tx);
        }
        _ => {
            // Render AI recommendations (default)
            render_recommendations_tab(ui, config, wisdom_gate_recommendations, tx);
        }
    }
}

fn render_recently_added_tab(ui: &mut egui::Ui, recently_added_items: &[Item], lang: Language, _tx: &Sender<Msg>) {
    if recently_added_items.is_empty() {
        ui.colored_label(egui::Color32::GRAY, t("loading_content", lang));
        ui.label(t("loading_newest", lang));
        
        // Trigger loading (this will be called from main.rs)
        return;
    }

    ui.label(egui::RichText::new(t("newly_added", lang))
        .strong()
        .size(16.0));
    ui.add_space(8.0);

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            for item in recently_added_items.iter().take(20) {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&item.name)
                            .strong()
                            .size(13.0));
                        if !item.plot.is_empty() {
                            let truncated = if item.plot.len() > 80 {
                                format!("{}...", &item.plot[..80])
                            } else {
                                item.plot.clone()
                            };
                            ui.label(egui::RichText::new(truncated)
                                .size(11.0)
                                .color(egui::Color32::GRAY));
                        }
                        if let Some(year) = &item.year {
                            ui.label(egui::RichText::new(format!("📅 {}", year))
                                .size(10.0)
                                .color(egui::Color32::DARK_GRAY));
                        }
                    });
                });
                ui.separator();
            }
        });
}

fn render_recommendations_tab(
    ui: &mut egui::Ui,
    config: &Config,
    wisdom_gate_recommendations: &Option<String>,
    tx: &Sender<Msg>,
) {
    // Check API Key based on selected provider
    let has_api_key = match config.ai_provider.as_str() {
        "perplexity" => !config.perplexity_api_key.is_empty(),
        "cognora" => !config.cognora_api_key.is_empty(),
        "gemini" => !config.gemini_api_key.is_empty(),
        "openai" => !config.openai_api_key.is_empty(),
        _ => !config.wisdom_gate_api_key.is_empty(),
    };

    // API Key Status
    if !has_api_key {
        ui.colored_label(egui::Color32::YELLOW, "⚠️ Kein API-Key konfiguriert");
        let provider_name = match config.ai_provider.as_str() {
            "perplexity" => "Perplexity",
            "cognora" => "Cognora Toolkit",
            "gemini" => "Gemini",
            "openai" => "OpenAI",
            _ => "Wisdom-Gate",
        };
        ui.label(format!("Bitte {} API-Key in den Einstellungen hinzufügen.", provider_name));
        ui.add_space(5.0);
        
        let (provider_icon, provider_display, model) = match config.ai_provider.as_str() {
            "perplexity" => ("🔮", "Perplexity", &config.perplexity_model),
            "cognora" => ("🧠", "Cognora Toolkit", &config.cognora_model),
            "gemini" => ("💎", "Gemini", &config.gemini_model),
            "openai" => ("🤖", "OpenAI", &config.openai_model),
            _ => ("🤖", "Wisdom-Gate", &config.wisdom_gate_model),
        };
        
        ui.label(format!("Provider: {} {}", provider_icon, provider_display));
        ui.label(format!("Model: {}", model));
        ui.label(format!("Prompt: {}", config.wisdom_gate_prompt.chars().take(50).collect::<String>() + "..."));
        return;
    }

    // Fetch recommendations button
    ui.horizontal(|ui| {
        if ui.button("🔄 Empfehlungen laden").clicked() {
            // Always fetch new content when button is clicked - ignore cache
            let tx = tx.clone();
            
            // Extract all needed values from config before moving into async block
            let provider = config.ai_provider.clone();
            let perplexity_api_key = config.perplexity_api_key.clone();
            let cognora_api_key = config.cognora_api_key.clone();
            let gemini_api_key = config.gemini_api_key.clone();
            let openai_api_key = config.openai_api_key.clone();
            let wisdom_gate_api_key = config.wisdom_gate_api_key.clone();
            let prompt = config.wisdom_gate_prompt.clone();
            let perplexity_model = config.perplexity_model.clone();
            let cognora_model = config.cognora_model.clone();
            let gemini_model = config.gemini_model.clone();
            let openai_model = config.openai_model.clone();
            let wisdom_gate_model = config.wisdom_gate_model.clone();
            let wisdom_gate_endpoint = config.wisdom_gate_endpoint.clone();
            
            tokio::spawn(async move {
                let content = match provider.as_str() {
                    "perplexity" => {
                        crate::api::fetch_perplexity_recommendations_safe(
                            &perplexity_api_key,
                            &prompt,
                            &perplexity_model
                        ).await
                    }
                    "cognora" => {
                        crate::api::fetch_cognora_recommendations_safe(
                            &cognora_api_key,
                            &prompt,
                            &cognora_model
                        ).await
                    }
                    "gemini" => {
                        crate::api::fetch_gemini_recommendations_safe(
                            &gemini_api_key,
                            &prompt,
                            &gemini_model
                        ).await
                    }
                    "openai" => {
                        crate::api::fetch_openai_recommendations_safe(
                            &openai_api_key,
                            &prompt,
                            &openai_model
                        ).await
                    }
                    _ => {
                        crate::api::fetch_wisdom_gate_recommendations_safe(
                            &wisdom_gate_api_key,
                            &prompt,
                            &wisdom_gate_model,
                            &wisdom_gate_endpoint
                        ).await
                    }
                };
                let _ = tx.send(Msg::WisdomGateRecommendations(content));
            });
        }

        // Show provider and cache status
        let (provider_icon, provider_display) = match config.ai_provider.as_str() {
            "perplexity" => ("🔮", "Perplexity"),
            "cognora" => ("🧠", "Cognora"),
            "gemini" => ("💎", "Gemini"),
            "openai" => ("🤖", "OpenAI"),
            _ => ("🤖", "Wisdom-Gate"),
        };
        ui.label(format!("{} {}", provider_icon, provider_display));
        
        if config.is_wisdom_gate_cache_valid() {
            let cache_age = config.get_wisdom_gate_cache_age_hours();
            ui.label(format!("📦 Cache: {}h alt", cache_age));
        } else if !config.wisdom_gate_cache_content.is_empty() {
            ui.colored_label(egui::Color32::YELLOW, "⚠️ Cache abgelaufen");
        } else {
            ui.colored_label(egui::Color32::GRAY, "📭 Kein Cache");
        }
    });

    ui.add_space(10.0);

    // Display recommendations
    if let Some(content) = wisdom_gate_recommendations {
        // Zusatz-Hinweis bei DNS / Endpoint Problemen
        if content.contains("DNS/Verbindungsfehler") || (content.contains("Endpoint:") && content.contains("nicht erreichbar")) {
            ui.colored_label(egui::Color32::RED, "🛑 Endpoint nicht erreichbar");
            ui.label("Die angegebene Wisdom-Gate API konnte nicht aufgelöst oder verbunden werden.");
            ui.label("Prüfe:");
            ui.label("• Internet / Firewall / VPN / Proxy");
            ui.label("• DNS Schreibweise der Domain");
            ui.label("• Alternative Domain ohne/mit Bindestrich testen");
            ui.add_space(4.0);
            ui.monospace(&config.wisdom_gate_endpoint);
            ui.add_space(6.0);
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(egui::RichText::new("🎬 Heutige Streaming-Empfehlungen:")
                .strong()
                .size(16.0));
            ui.add_space(8.0);
            
            if content.starts_with("Fehler") {
                ui.colored_label(egui::Color32::RED, 
                    egui::RichText::new(content).size(14.0));
            } else {
                // Parse and display with larger font and selectable text
                for line in content.lines() {
                    if line.trim().is_empty() {
                        ui.add_space(4.0);
                        continue;
                    }
                    
                    // Headers (### or ##)
                    if line.starts_with("###") || line.starts_with("##") {
                        let _ = ui.selectable_label(false, egui::RichText::new(line.trim_start_matches('#').trim())
                            .strong()
                            .size(18.0)
                            .color(egui::Color32::from_rgb(100, 200, 255)));
                        ui.add_space(3.0);
                    } 
                    // Bold text (**text**)
                    else if line.starts_with("**") && line.ends_with("**") {
                        let _ = ui.selectable_label(false, egui::RichText::new(line.trim_start_matches("**").trim_end_matches("**"))
                            .strong()
                            .size(15.0)
                            .color(egui::Color32::from_rgb(255, 255, 150)));
                        ui.add_space(2.0);
                    } 
                    // List items or content with bullets
                    else if line.starts_with("*") || line.starts_with("-") || line.contains("–") {
                        let _ = ui.selectable_label(false, egui::RichText::new(line.trim_start_matches('*').trim_start_matches('-').trim())
                            .size(14.0)
                            .color(egui::Color32::LIGHT_GRAY));
                        ui.add_space(1.0);
                    } 
                    // Regular text
                    else {
                        let _ = ui.selectable_label(false, egui::RichText::new(line)
                            .size(14.0)
                            .color(egui::Color32::LIGHT_GRAY));
                        ui.add_space(1.0);
                    }
                }
            }
        });
    } else {
        ui.colored_label(egui::Color32::GRAY, "📭 Noch keine Empfehlungen geladen...");
        ui.label("Klicken Sie auf 'Empfehlungen aktualisieren' um zu starten.");
    }
}
