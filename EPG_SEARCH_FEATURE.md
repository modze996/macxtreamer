# EPG-Suche Feature

## Übersicht
Die normale Suche in MacXtreamer wurde erweitert, um auch nach TV-Programmen (EPG-Daten) zu suchen. Diese Funktionalität ermöglicht es Benutzern, nach spezifischen TV-Sendungen, Filmen im Fernsehen oder Programmen zu suchen.

## Neue Funktionalität

### 1. Erweiterte Suchfunktion
- **Datei**: `src/search.rs`
- **Neue Funktion**: `search_items_with_epg()`
- **Suchbereiche**:
  - Filme (wie bisher)
  - Serien (wie bisher)
  - **NEU**: TV-Programme aus EPG-Daten

### 2. EPG-Suchergebnisse
TV-Programme werden durchsucht nach:
- **Titel** des Programms
- **Beschreibung** der Sendung
- **Kategorie** (Genre)

### 3. Suchergebnis-Format für TV-Programme
- **Name**: `"Programm-Titel (Startzeit) - Beschreibung"`
- **Info**: `"TV Program on Kanal-Name"`
- **ID**: `"epg_kanal-id_programm-id"` (spezielles Format)
- **Genre**: Kategorie aus EPG-Daten (falls vorhanden)

### 4. Abspiel-Logik für EPG-Ergebnisse
- **Datei**: `src/main.rs`, Funktion `resolve_play_url()`
- EPG-basierte Suchergebnisse (ID beginnt mit "epg_") werden automatisch zum entsprechenden Live-Kanal weitergeleitet
- Beim Klicken auf "Play" für ein TV-Programm wird der Live-Stream des Kanals gestartet

## Technische Implementierung

### Dependencies
- **Chrono** (neu hinzugefügt): Für Zeitformatierung der EPG-Startzeiten
  ```toml
  chrono = "0.4"
  ```

### Code-Änderungen

#### 1. search.rs - Neue Suchfunktion
```rust
pub fn search_items_with_epg(
    movies: &Vec<Item>, 
    series: &Vec<Item>, 
    epg_events: &HashMap<String, Vec<EpgEvent>>,
    live_channels: &[(String, String)], 
    text: &str
) -> Vec<SearchItem>
```

#### 2. main.rs - Integration
- **start_search()**: Verwendet neue EPG-Suchfunktion
- **resolve_play_url()**: Behandelt EPG-basierte IDs
- Import: `use search::search_items_with_epg;`

## Benutzer-Experience

### Vor der Erweiterung
- Suche nach "Tatort" → Nur Filme/Serien namens "Tatort"

### Nach der Erweiterung  
- Suche nach "Tatort" → 
  - Filme/Serien namens "Tatort"
  - **PLUS**: Alle TV-Programme mit "Tatort" im Titel/Beschreibung
  - Beispiel: "Tatort: Borowski und das Land zwischen den Meeren (20:15) - Krimi aus Kiel"

### Suchergebnis-Anzeige
```
📺 TV Program on Das Erste
Tatort: Borowski und das Land zwischen den Meeren (20:15) - Krimi aus Kiel...
[Play] [EPG] [Favorite]
```

## Testing

### Manuelle Tests
1. **EPG-Daten laden**: Live-Kanäle öffnen → EPG-Button klicken
2. **Suche testen**: Nach bekannten Programmnamen suchen
3. **Abspiel-Test**: Play-Button bei EPG-Ergebnissen testen

### Test-Szenarien
- Suche nach Programm-Titel (z.B. "Tagesschau")
- Suche nach Genre (z.B. "Krimi", "Nachrichten")
- Suche nach Beschreibungstext
- Gemischte Ergebnisse (Filme + EPG) prüfen

## Performance-Überlegungen

### Rate Limiting
- EPG-Daten werden nur einmal pro Session automatisch geladen
- Verhindert übermäßige Server-Anfragen
- Existierende EPG-Daten werden für Suche wiederverwendet

### Speicher-Effizienz
- EPG-Daten werden im Speicher gehalten
- Suchalgorithmus durchsucht nur geladene Daten
- Keine zusätzlichen Netzwerk-Requests bei Suche

## Fehlerbehebung

### Keine EPG-Suchergebnisse
1. EPG-Daten geladen? → Live-Kanäle besuchen
2. Suchbegriff zu spezifisch? → Allgemeinere Begriffe testen
3. Kanal-EPG verfügbar? → EPG-Button bei Live-Kanälen testen

### Play-Button funktioniert nicht bei EPG-Ergebnissen
1. Kanal-ID korrekt extrahiert? → Debug-Logs prüfen
2. Live-Stream verfügbar? → Direkten Kanal-Zugriff testen

## Zukunftige Erweiterungen

### Mögliche Verbesserungen
1. **Zeitbasierte Suche**: "heute Abend", "20:15"
2. **Erweiterte Filter**: Nach Kanal, Startzeit, Dauer
3. **EPG-Kategorien**: Dedicated Genre-Filter für TV
4. **Favoriten**: EPG-Programme als Favoriten speichern
5. **Benachrichtigungen**: Für kommende Programme

### Performance-Optimierungen
1. **Indexierung**: EPG-Daten für schnellere Suche indizieren
2. **Caching**: Suchergebnisse zwischen Sessions cachen
3. **Partielle Suche**: Nur relevante Kanäle durchsuchen

## Kompatibilität
- **macOS**: ✅ Getestet
- **Abhängigkeiten**: Kompatibel mit existierenden Dependencies
- **Breaking Changes**: Keine - vollständig rückwärtskompatibel