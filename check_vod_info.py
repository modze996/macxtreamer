#!/usr/bin/env python3
import requests
import json

try:
    with open('/Users/mamo/Library/Application Support/MacXtreamer/xtream_config.txt', 'r') as f:
        lines = f.readlines()
        config = {}
        for line in lines:
            if '=' in line:
                key, value = line.strip().split('=', 1)
                config[key] = value
        
        address = config.get('address', '')
        username = config.get('username', '')
        password = config.get('password', '')
        
        # Hole ein VOD Item
        print("Fetching VOD categories...")
        url = f"{address}/player_api.php?username={username}&password={password}&action=get_vod_categories"
        response = requests.get(url, timeout=10)
        cats = response.json()
        cat_id = cats[0].get('category_id', cats[0].get('id', '1'))
        
        # Hole Items
        url = f"{address}/player_api.php?username={username}&password={password}&action=get_vod_streams&category_id={cat_id}"
        response = requests.get(url, timeout=10)
        data = response.json()
        
        if data and len(data) > 0:
            vod_id = data[0].get('stream_id')
            print(f"Checking detailed info for VOD ID: {vod_id}")
            print(f"VOD Name: {data[0].get('name')}")
            
            # Hole Detail-Info
            url = f"{address}/player_api.php?username={username}&password={password}&action=get_vod_info&vod_id={vod_id}"
            response = requests.get(url, timeout=10)
            if response.status_code == 200:
                detail = response.json()
                print("\n📋 VOD Detail Info Structure:")
                print(json.dumps(detail, indent=2)[:2000])  # Erste 2000 Zeichen
                
                # Suche nach language-bezogenen Feldern
                def find_lang_fields(obj, path=""):
                    results = []
                    if isinstance(obj, dict):
                        for key, value in obj.items():
                            current_path = f"{path}.{key}" if path else key
                            if 'lang' in key.lower() or 'audio' in key.lower():
                                results.append((current_path, value))
                            results.extend(find_lang_fields(value, current_path))
                    elif isinstance(obj, list):
                        for i, item in enumerate(obj):
                            results.extend(find_lang_fields(item, f"{path}[{i}]"))
                    return results
                
                lang_fields = find_lang_fields(detail)
                if lang_fields:
                    print("\n🎯 Language-related fields found:")
                    for path, value in lang_fields:
                        print(f"  {path} = {value}")
                else:
                    print("\n❌ No language-related fields in detailed info")
                        
except Exception as e:
    print(f"Error: {e}")
    import traceback
    traceback.print_exc()
