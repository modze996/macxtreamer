import { Config, getCacheTtlMs } from "./config";

type CacheEntry<T> = {
  data: T;
  expiresAt: number;
};

const memoryCache = new Map<string, CacheEntry<any>>();

function buildAccountKey(config: Config): string {
  return `${config.address}::${config.username}`;
}

export function buildCacheKey(
  config: Config,
  scope: string,
  params: Record<string, string | number | null | undefined> = {}
): string {
  const sortedParams = Object.entries(params)
    .filter(([, value]) => value !== undefined)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([key, value]) => `${key}=${value ?? ""}`)
    .join("&");

  return `${buildAccountKey(config)}::${scope}::${sortedParams}`;
}

export function getFromCache<T>(key: string): T | null {
  const entry = memoryCache.get(key);
  if (!entry) return null;

  const now = Date.now();
  if (entry.expiresAt > now) {
    return entry.data as T;
  }

  memoryCache.delete(key);
  return null;
}

export function setCache<T>(key: string, data: T, ttlMs: number): void {
  const expiresAt = Date.now() + ttlMs;
  memoryCache.set(key, { data, expiresAt });
}

export async function getOrFetch<T>(
  config: Config,
  scope: string,
  params: Record<string, string | number | null | undefined> = {},
  fetcher: () => Promise<T>,
  options: { forceRefresh?: boolean; ttlMs?: number } = {}
): Promise<T> {
  const cacheKey = buildCacheKey(config, scope, params);
  const ttlMs = options.ttlMs ?? getCacheTtlMs(config);
  const forceRefresh = options.forceRefresh === true;

  if (!forceRefresh) {
    const cached = getFromCache<T>(cacheKey);
    if (cached !== null) {
      return cached;
    }
  }

  const data = await fetcher();
  setCache(cacheKey, data, ttlMs);
  return data;
}

export function clearAccountCache(config: Config) {
  const accountPrefix = `${buildAccountKey(config)}::`;
  for (const key of memoryCache.keys()) {
    if (key.startsWith(accountPrefix)) {
      memoryCache.delete(key);
    }
  }
}
