import fs from "fs";
import path from "path";
import os from "os";

export interface Config {
  address: string;
  username: string;
  password: string;
  /** Optional override in days for API response cache TTL */
  cacheDays?: number;
}

let cachedConfig: Config | null = null;

function parseTomlConfig(content: string): Config | null {
  // Try TOML format first
  let addressMatch = content.match(/address\s*=\s*['\"](.*?)['\"]/);
  let usernameMatch = content.match(/username\s*=\s*['\"](.*?)['\"]/);
  let passwordMatch = content.match(/password\s*=\s*['\"](.*?)['\"]/);
  let cacheDaysMatch = content.match(/cache_days\s*=\s*['\"]?(\d+)['\"]?/);

  // If TOML format doesn't work, try plain key=value format (from Rust app)
  if (!addressMatch || !usernameMatch || !passwordMatch) {
    addressMatch = content.match(/^address\s*=\s*(.+?)$/m);
    usernameMatch = content.match(/^username\s*=\s*(.+?)$/m);
    passwordMatch = content.match(/^password\s*=\s*(.+?)$/m);
    cacheDaysMatch = content.match(/^cache_days\s*=\s*(\d+)$/m);
  }

  if (addressMatch && usernameMatch && passwordMatch) {
    return {
      address: addressMatch[1].trim().replace(/^['\"]|['\"]$/g, ''),
      username: usernameMatch[1].trim().replace(/^['\"]|['\"]$/g, ''),
      password: passwordMatch[1].trim().replace(/^['\"]|['\"]$/g, ''),
      cacheDays: cacheDaysMatch ? Number(cacheDaysMatch[1]) : undefined,
    };
  }

  return null;
}

export async function loadConfig(): Promise<Config | null> {
  if (cachedConfig) return cachedConfig;

  try {
    const homeDir = os.homedir();
    const pathsToTry = [
      // Standard macxtreamer location (where Rust app saves it)
      path.join(homeDir, "Library", "Application Support", "MacXtreamer", "xtream_config.txt"),
      // Alternative TOML format
      path.join(homeDir, ".config", "macxtreamer", "config.toml"),
      // Fallback for development
      path.join(homeDir, ".macxtreamer", "config.toml"),
      // Next.js project root parent (development)
      path.join(process.cwd(), "..", "config.toml"),
      // Current working directory (development)
      path.join(process.cwd(), "config.toml"),
      // Legacy text format in home directory
      path.join(homeDir, "xtream_config.txt"),
    ];

    console.log("[Config] Searching for config at paths:");
    for (const configPath of pathsToTry) {
      console.log(`  - ${configPath}`);
      
      if (fs.existsSync(configPath)) {
        console.log(`  ✓ Found config at: ${configPath}`);
        const content = fs.readFileSync(configPath, "utf-8");
        const config = parseTomlConfig(content);
        
        if (config) {
          console.log(`  ✓ Config parsed successfully`);
          cachedConfig = config;
          return config;
        } else {
          console.log(`  ✗ Failed to parse config at ${configPath}`);
        }
      }
    }

    console.log("[Config] No valid config file found in any of the paths");
    return null;
  } catch (error) {
    console.error("[Config] Error loading config:", error);
    return null;
  }
}

export async function getConfig(): Promise<Config> {
  const config = await loadConfig();
  if (!config) {
    throw new Error(
      "Configuration not found.\n\n" +
      "Please ensure MacXtreamer desktop app has been configured first, or create a config file at:\n" +
      "~/Library/Application Support/MacXtreamer/xtream_config.txt\n\n" +
      "Format:\n" +
      "address=http://your-iptv-server.com\n" +
      "username=your_username\n" +
      "password=your_password"
    );
  }
  return config;
}

const DEFAULT_CACHE_DAYS = 5;

function resolveCacheDays(config?: Config): number {
  const envValue =
    process.env.MACXTREAMER_CACHE_DAYS || process.env.NEXT_PUBLIC_MACXTREAMER_CACHE_DAYS;

  if (envValue && !Number.isNaN(Number(envValue))) {
    return Math.max(0, Number(envValue));
  }

  if (config?.cacheDays !== undefined && !Number.isNaN(Number(config.cacheDays))) {
    return Math.max(0, Number(config.cacheDays));
  }

  return DEFAULT_CACHE_DAYS;
}

export function getCacheTtlMs(config?: Config): number {
  const days = resolveCacheDays(config);
  return days * 24 * 60 * 60 * 1000;
}
