# Fix: 500% CPU-Auslastung durch JustWatch Rate-Limiting

## 🚨 Problem
Nach der EPG-Suche-Implementierung kam es wieder zu 500% CPU-Auslastung durch unkontrollierte JustWatch-Empfehlungsanfragen.

## 🔍 Ursache
**Endlosschleife in `render_justwatch_panel()`**:
```rust
// PROBLEMATISCH:
if self.justwatch_recommendations.is_empty() && needs_refresh {
    // Spawnt kontinuierlich neue Tasks ohne Rate-Limiting
    tokio::spawn(async move { ... });
}
```

### Konkrete Probleme:
1. **Inkonsistente Rate-Limits**: `reload_categories()` verwendete 5 Minuten, `render_justwatch_panel()` 1 Stunde
2. **Fehlende Sofortsperre**: Kein `justwatch_last_fetch` beim Task-Spawn setzen
3. **Ungeschützter Aktualisieren-Button**: Keine Rate-Limiting beim manuellen Refresh

## ✅ Lösung implementiert

### 1. **Konsistente Rate-Limiting-Zeiten**
```rust
// VORHER: 3600 Sekunden (1 Stunde)
.map(|last| last.elapsed() > Duration::from_secs(3600))

// NACHHER: 300 Sekunden (5 Minuten) - konsistent mit reload_categories
.map(|last| last.elapsed() > Duration::from_secs(300))
```

### 2. **Sofortige Task-Sperre**
```rust
if self.justwatch_recommendations.is_empty() && needs_refresh {
    // KRITISCH: Sofort setzen um mehrfache Spawns zu verhindern
    self.justwatch_last_fetch = Some(std::time::Instant::now());
    
    tokio::spawn(async move { ... });
}
```

### 3. **Geschützter Aktualisieren-Button**
```rust
if ui.button("🔄 Aktualisieren").clicked() {
    let can_refresh = self.justwatch_last_fetch
        .map(|last| last.elapsed() > Duration::from_secs(300))
        .unwrap_or(true);
    
    if can_refresh {
        self.justwatch_last_fetch = Some(std::time::Instant::now());
        // Nur dann spawnen...
    }
}
```

## 📊 Ergebnis

### Vorher (CPU-Problem):
```
📊 Loading Top 10 from multiple sources (Movies: 10, Series: 10)
✅ TMDB trending data loaded: 20 items
📊 Loading Top 10 from multiple sources (Movies: 10, Series: 10)  ← Endlos
✅ TMDB trending data loaded: 20 items                              ← Wiederholt
📊 Loading Top 10 from multiple sources (Movies: 10, Series: 10)  ← Kontinuierlich
...
```

### Nachher (Problem behoben):
```
📊 Loading Top 10 from multiple sources (Movies: 10, Series: 10)
✅ TMDB trending data loaded: 20 items
📊 Loading Top 10 from multiple sources (Movies: 10, Series: 10)
✅ TMDB trending data loaded: 20 items
� Total recommendations extracted: 10
✅ JustWatch validation successful
✅ Final dataset: 20 recommendations from hybrid sources
� Total recommendations extracted: 10
✅ JustWatch validation successful
✅ Final dataset: 20 recommendations from hybrid sources
                                                                 ← Stopp! Keine weiteren Requests
```

## 🛡️ Präventive Maßnahmen

### Rate-Limiting-Prinzipien:
1. **Konsistente Zeitfenster**: Alle JustWatch-Operationen verwenden 5-Minuten-Rate-Limiting
2. **Sofortige Sperre**: `justwatch_last_fetch` wird VOR dem Task-Spawn gesetzt
3. **Umfassender Schutz**: Automatische UND manuelle Triggers sind geschützt
4. **Graceful Degradation**: Fehlende Daten werden elegant behandelt

### Getestete Szenarien:
- ✅ App-Start: Lädt JustWatch-Daten einmalig
- ✅ UI-Navigation: Keine zusätzlichen Requests
- ✅ Manueller Refresh: Respektiert 5-Minuten-Limit
- ✅ Leere Daten: Verhindert Endlosschleifen

## 🎯 Performance-Monitoring

### Normale CPU-Auslastung erwartet:
- **App-Start**: Kurzzeitig erhöht (Initialisierung)
- **Normalbetrieb**: Niedrig (~5-15%)
- **Nach 2 JustWatch-Requests**: Stabil, keine weiteren Netzwerk-Aktivitäten

### Warnsignale für CPU-Probleme:
- Kontinuierliche "📊 Loading Top 10..." Nachrichten
- Wiederholende TMDB-Anfragen ohne Pause
- Terminal-Output läuft ohne zu stoppen

## 🔧 Code-Standorte

**Geänderte Funktionen**:
- `render_justwatch_panel()` - Rate-Limiting bei automatischem Laden
- Aktualisieren-Button - Rate-Limiting bei manuellem Refresh

**Rate-Limiting-Konfiguration**:
- Alle JustWatch-Operationen: **5 Minuten** (300 Sekunden)
- Tracking-Variable: `justwatch_last_fetch: Option<std::time::Instant>`

## ✅ Status: Problem behoben

Die 500% CPU-Auslastung durch unkontrollierte JustWatch-Requests wurde erfolgreich eliminiert. Die Anwendung läuft wieder stabil mit normalem CPU-Verbrauch.