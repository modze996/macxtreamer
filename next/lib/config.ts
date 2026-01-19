import fs from "fs";
import path from "path";
import os from "os";

export interface Config {
  address: string;
  username: string;
  password: string;
}

let cachedConfig: Config | null = null;

export async function loadConfig(): Promise<Config | null> {
  if (cachedConfig) return cachedConfig;

  try {
    // Try to load from macxtreamer config file
    const homeDir = os.homedir();
    const configPath = path.join(
      homeDir,
      ".config",
      "macxtreamer",
      "config.toml"
    );

    if (fs.existsSync(configPath)) {
      const content = fs.readFileSync(configPath, "utf-8");

      // Parse TOML manually (simple parser for our use case)
      const addressMatch = content.match(/address\s*=\s*['"](.*?)['"]/);
      const usernameMatch = content.match(/username\s*=\s*['"](.*?)['"]/);
      const passwordMatch = content.match(/password\s*=\s*['"](.*?)['"]/);

      if (addressMatch && usernameMatch && passwordMatch) {
        cachedConfig = {
          address: addressMatch[1],
          username: usernameMatch[1],
          password: passwordMatch[1],
        };
        return cachedConfig;
      }
    }

    // Try alternative location for development
    const devConfigPath = path.join(
      process.cwd(),
      "..",
      "config.toml"
    );
    if (fs.existsSync(devConfigPath)) {
      const content = fs.readFileSync(devConfigPath, "utf-8");
      const addressMatch = content.match(/address\s*=\s*['"](.*?)['"]/);
      const usernameMatch = content.match(/username\s*=\s*['"](.*?)['"]/);
      const passwordMatch = content.match(/password\s*=\s*['"](.*?)['"]/);

      if (addressMatch && usernameMatch && passwordMatch) {
        cachedConfig = {
          address: addressMatch[1],
          username: usernameMatch[1],
          password: passwordMatch[1],
        };
        return cachedConfig;
      }
    }
  } catch (error) {
    console.error("Error loading config:", error);
  }

  return null;
}

export async function getConfig(): Promise<Config> {
  const config = await loadConfig();
  if (!config) {
    throw new Error(
      "Configuration not found. Please configure macxtreamer first."
    );
  }
  return config;
}
