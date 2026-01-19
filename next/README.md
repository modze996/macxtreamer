# MacXStreamer Web

Web frontend für MacXStreamer IPTV Player mit Next.js.

## Features

- 📺 Live TV Kanäle
- 🎬 Video on Demand (VOD)
- 📺 Series mit Episode-Navigation
- 🔄 Automatische Konfigurationsintegration
- 🎨 Modernes UI mit Tailwind CSS

## Installation

```bash
npm install
```

## Konfiguration

Die Anwendung liest automatisch die MacXStreamer-Konfiguration aus:
- `~/.config/macxtreamer/config.toml`

Die Datei sollte folgende Einträge enthalten:

```toml
address = "http://your-iptv-server.com"
username = "your_username"
password = "your_password"
```

## Entwicklung

```bash
npm run dev
```

Die Anwendung läuft dann auf `http://localhost:3000`

## Build

```bash
npm run build
npm start
```

## API Endpoints

- `GET /api/config` - Konfigurationsstatus
- `GET /api/categories?action=<action>` - Kategorien abrufen
- `GET /api/items?action=<action>&category_id=<id>` - Items einer Kategorie
- `GET /api/episodes?series_id=<id>` - Episoden einer Serie

Unterstützte Actions:
- `get_live_categories` / `get_live_streams`
- `get_vod_categories` / `get_vod_streams`
- `get_series_categories` / `get_series`
