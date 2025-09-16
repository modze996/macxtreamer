import requests
import json

# Versuche verschiedene category IDs
url_template = "http://xcpanel.live:8080/player_api.php?username=xxxxx&password=xxxxx&action=get_vod_streams&category_id=122"

# Simuliere die Struktur basierend auf dem bereitgestellten Beispiel
sample_data = {
    "num": 1,
    "name": "Die Trickbetrügerin (2025) DE",
    "series_id": 17599,
    "cover": "https://image.tmdb.org/t/p/w600_and_h900_bestv2/dmNslVCURU1Oq04xOu7hAURmYWQ.jpg",
    "plot": "Yoon Yi-Rang ist eine brillante Betrügerin...",
    "cast": "박민영, 박희순, 주종혁",
    "director": "",
    "genre": "Krimi / Komödie / Drama",
    "releaseDate": "2025-09-06",
    "release_date": "2025-09-06",
    "last_modified": "1757960997",
    "rating": "8",
    "rating_5based": "4",
    "backdrop_path": [
        "https://image.tmdb.org/t/p/w1280/ibT5W5lpedfG9TUmplkMldpDigJ.jpg"
    ],
    "youtube_trailer": "",
    "tmdb": "259710",
    "episode_run_time": "0",
    "category_id": "135",
    "category_ids": [135]
}

print("Beispiel der erwarteten API-Struktur:")
print(json.dumps(sample_data, indent=2))
