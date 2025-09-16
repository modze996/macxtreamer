import requests
import json
import sys

# Lese Config aus der Datei
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
        
        # API call für get_series mit category_id=135
        url = f"{address}/player_api.php?username={username}&password={password}&action=get_series&category_id=135"
        
        print("Making API call to:", url.replace(username, "***").replace(password, "***"))
        
        response = requests.get(url)
        if response.status_code == 200:
            data = response.json()
            print("\nFirst item from API response:")
            print(json.dumps(data[0] if data else {}, indent=2))
        else:
            print(f"Error: {response.status_code}")
            
except Exception as e:
    print(f"Error: {e}")
