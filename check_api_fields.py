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
        
        # Erst Kategorien holen
        print("Fetching VOD categories...")
        url = f"{address}/player_api.php?username={username}&password={password}&action=get_vod_categories"
        response = requests.get(url, timeout=10)
        if response.status_code == 200:
            cats = response.json()
            if cats and len(cats) > 0:
                cat_id = cats[0].get('category_id', cats[0].get('id', '1'))
                print(f"Using category: {cats[0].get('category_name', 'Unknown')} (ID: {cat_id})")
                
                # Jetzt Items aus dieser Kategorie holen
                url = f"{address}/player_api.php?username={username}&password={password}&action=get_vod_streams&category_id={cat_id}"
                response = requests.get(url, timeout=10)
                if response.status_code == 200:
                    data = response.json()
                    if data and len(data) > 0:
                        print(f"\n✅ Found {len(data)} items")
                        print("\nAll fields in first item:")
                        first_item = data[0]
                        for key in sorted(first_item.keys()):
                            value = first_item[key]
                            if isinstance(value, str) and len(value) > 100:
                                print(f"  {key}: <string, {len(value)} chars>")
                            else:
                                print(f"  {key}: {value}")
                        
                        # Suche nach language-bezogenen Feldern
                        lang_fields = [k for k in first_item.keys() if 'lang' in k.lower() or 'audio' in k.lower()]
                        if lang_fields:
                            print(f"\n🎯 Language-related fields: {lang_fields}")
                            for field in lang_fields:
                                print(f"  {field} = {first_item[field]}")
                        else:
                            print("\n❌ No language-related fields found")
                    else:
                        print("No items in this category")
                        
except Exception as e:
    print(f"Error: {e}")
    import traceback
    traceback.print_exc()
