#!/usr/bin/env python3
import requests
import json

# Lese Config
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
        
        # Teste verschiedene Endpunkte
        endpoints = [
            ('get_live_streams', '1'),
            ('get_vod_streams', '1'),
        ]
        
        for action, cat_id in endpoints:
            url = f"{address}/player_api.php?username={username}&password={password}&action={action}&category_id={cat_id}"
            print(f"\n{'='*60}")
            print(f"Testing: {action}")
            print(f"{'='*60}")
            
            try:
                response = requests.get(url, timeout=10)
                if response.status_code == 200:
                    data = response.json()
                    if data and len(data) > 0:
                        print(f"Found {len(data)} items")
                        print("\nFirst item fields:")
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
                            print(f"\n🎯 Language-related fields found: {lang_fields}")
                        break
                    else:
                        print("No items returned")
                else:
                    print(f"Error: {response.status_code}")
            except Exception as e:
                print(f"Error: {e}")
                
except Exception as e:
    print(f"Config error: {e}")
