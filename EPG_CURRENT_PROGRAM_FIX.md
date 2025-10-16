# Fix: EPG Programm-Info in Suchergebnissen

## Problem
EPG-basierte Suchergebnisse zeigten immer noch "no program info" an, obwohl sie eigentlich die aktuelle Programminformation anzeigen sollten.

## Ursache
In der `start_search()` Funktion wurde für alle Suchergebnisse `current_program: None` gesetzt, ohne zwischen normalen Inhalten (Filme/Serien) und EPG-basierten Ergebnissen zu unterscheiden.

## Lösung
**Datei**: `src/main.rs`, Funktion `start_search()`

### Vorher:
```rust
current_program: None, // Search items don't have current programs
```

### Nachher:
```rust
// For EPG-based search results, extract channel ID and get current program
let current_program = if s.id.starts_with("epg_") {
    if let Some(channel_id) = s.id.strip_prefix("epg_").and_then(|s| s.split('_').next()) {
        epg_events.get(channel_id)
            .and_then(|events| get_current_program(events))
    } else {
        None
    }
} else {
    None // Movies and series don't have current programs
};
```

## Technische Details

### EPG-ID Format
- EPG-Suchergebnisse haben IDs im Format: `epg_kanal-id_programm-id`
- Der Code extrahiert die `kanal-id` aus der EPG-ID
- Mit der `kanal-id` wird in `epg_events` nach den aktuellen Programmdaten gesucht

### Aktuelle Programm-Anzeige
- `get_current_program(events)` findet das aktuell laufende Programm
- Falls kein aktuelles Programm läuft, wird das nächste kommende Programm angezeigt
- Format: `"Programm-Titel (bis HH:MM)"` oder `"Nächstes: Programm-Titel (HH:MM)"`

## Ergebnis
✅ EPG-Suchergebnisse zeigen jetzt korrekt das aktuelle TV-Programm an  
✅ Normale Filme/Serien-Suchergebnisse bleiben unverändert  
✅ Keine Breaking Changes - vollständig rückwärtskompatibel  

## Test-Verifikation
1. **EPG-Daten laden**: Live-Kanäle öffnen → EPG-Button klicken
2. **Suche durchführen**: Nach Programmnamen suchen (z.B. "Tagesschau")
3. **Ergebnis prüfen**: EPG-Suchergebnisse sollten aktuelle Programm-Info anzeigen

### Beispiel-Anzeige:
```
📺 TV Program on Das Erste
Tatort: Borowski und das Land zwischen den Meeren (20:15) - Krimi...
Current Program: Tagesschau (bis 20:15)
[Play] [EPG] [Favorite]
```