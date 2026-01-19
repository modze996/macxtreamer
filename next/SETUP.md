# MacXtreamer Next.js Frontend Setup

## Configuration

The web frontend requires macxtreamer to be configured first. You have two options:

### Option 1: Configure via MacXtreamer Desktop App (Recommended)

1. Start the MacXtreamer desktop application
2. Enter your IPTV server details:
   - **Server Address**: e.g., `http://your-server.com:8000`
   - **Username**: Your IPTV username
   - **Password**: Your IPTV password
3. Click "Save" to store configuration
4. The config will be saved to: `~/Library/Application Support/MacXtreamer/xtream_config.txt`
5. Start the Next.js dev server - it will automatically find and use this configuration

### Option 2: Manual Configuration

Create a config file manually at:
```
~/Library/Application Support/MacXtreamer/xtream_config.txt
```

Add the following content (replace with your actual credentials):
```
address=http://your-iptv-server.com:8000
username=your_username
password=your_password
```

## Running the Frontend

```bash
cd next
npm run dev
```

The application will be available at `http://localhost:3000`

## Debugging

If you see "Configuration not found" error:

1. **Check if config exists:**
   ```bash
   cat ~/Library/Application Support/MacXtreamer/xtream_config.txt
   ```

2. **Check Next.js terminal output** for debug logs starting with `[Config]`

3. **Verify file permissions:**
   ```bash
   ls -la ~/Library/Application\ Support/MacXtreamer/
   ```

4. **Manual test - Create test config:**
   ```bash
   mkdir -p ~/Library/Application\ Support/MacXtreamer
   echo "address=http://localhost:8000" > ~/Library/Application\ Support/MacXtreamer/xtream_config.txt
   echo "username=test" >> ~/Library/Application\ Support/MacXtreamer/xtream_config.txt
   echo "password=test" >> ~/Library/Application\ Support/MacXtreamer/xtream_config.txt
   ```

## Supported Config Locations

The app searches for config in this priority order:

1. `~/Library/Application Support/MacXtreamer/xtream_config.txt` (macOS, from Rust app)
2. `~/.config/macxtreamer/config.toml` (Linux/fallback)
3. `~/.macxtreamer/config.toml` (fallback)
4. `./config.toml` (development)
5. `~/xtream_config.txt` (legacy)

## API Endpoints

Once configured, the following API endpoints are available:

- `GET /api/config` - Check configuration status
- `GET /api/categories` - List all categories
- `GET /api/items?cat_id=ID` - List items in a category
- `GET /api/episodes?vod_id=ID` - List episodes (for VOD)

