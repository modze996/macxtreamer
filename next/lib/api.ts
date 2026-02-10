// Utility functions for fetching data from the API

export interface Category {
  id: string;
  name: string;
}

export interface StreamItem {
  id: string;
  name: string;
  image?: string;
  plot?: string;
  containerExtension?: string;
  streamUrl?: string | null;
  year?: string | null;
  rating?: string | null;
  genre?: string | null;
  director?: string | null;
  cast?: string | null;
  audioLanguages?: string | null;
}

export interface ContentItem {
  id: string;
  title: string;
  subtitle?: string;
  coverUrl?: string;
  ranking?: number;
}

export interface ContinueWatchingItem {
  id: string;
  title: string;
  subtitle?: string;
  thumbnailUrl?: string;
  progress: number;
  type: "channel" | "series" | "movie";
}

export async function fetchCategories(
  type: "live" | "vod" | "series"
): Promise<Category[]> {
  try {
    const actionMap = {
      live: "get_live_categories",
      vod: "get_vod_categories",
      series: "get_series_categories",
    };
    
    const response = await fetch(`/api/categories?action=${actionMap[type]}`);
    if (!response.ok) {
      console.error(`Failed to fetch ${type} categories:`, response.status);
      return [];
    }
    
    return await response.json();
  } catch (error) {
    console.error(`Error fetching ${type} categories:`, error);
    return [];
  }
}

export async function fetchItems(
  type: "live" | "vod" | "series",
  categoryId: string
): Promise<StreamItem[]> {
  try {
    const actionMap = {
      live: "get_live_streams",
      vod: "get_vod_streams",
      series: "get_series_streams",
    };
    
    const response = await fetch(
      `/api/items?action=${actionMap[type]}&category_id=${categoryId}`
    );
    
    if (!response.ok) {
      console.error(`Failed to fetch ${type} items:`, response.status);
      return [];
    }
    
    return await response.json();
  } catch (error) {
    console.error(`Error fetching ${type} items:`, error);
    return [];
  }
}

export function convertStreamToContentItem(
  item: StreamItem,
  ranking?: number
): ContentItem {
  return {
    id: item.id,
    title: item.name,
    subtitle: item.year ? `${item.year}` : item.genre || undefined,
    coverUrl: item.image,
    ranking,
  };
}

export function convertStreamToContinueWatching(
  item: StreamItem,
  progress: number,
  type: "channel" | "series" | "movie"
): ContinueWatchingItem {
  return {
    id: item.id,
    title: item.name,
    subtitle: item.genre || item.year || undefined,
    thumbnailUrl: item.image,
    progress,
    type,
  };
}

// Storage keys for localStorage
const STORAGE_KEYS = {
  CONTINUE_WATCHING: "macxtreamer_continue_watching",
  FAVORITES: "macxtreamer_favorites",
};

export function getContinueWatching(): ContinueWatchingItem[] {
  if (typeof window === "undefined") return [];
  
  try {
    const data = localStorage.getItem(STORAGE_KEYS.CONTINUE_WATCHING);
    return data ? JSON.parse(data) : [];
  } catch (error) {
    console.error("Error loading continue watching:", error);
    return [];
  }
}

export function saveContinueWatching(items: ContinueWatchingItem[]): void {
  if (typeof window === "undefined") return;
  
  try {
    localStorage.setItem(STORAGE_KEYS.CONTINUE_WATCHING, JSON.stringify(items));
  } catch (error) {
    console.error("Error saving continue watching:", error);
  }
}

export function addToContinueWatching(item: ContinueWatchingItem): void {
  const items = getContinueWatching();
  const existingIndex = items.findIndex((i) => i.id === item.id);
  
  if (existingIndex >= 0) {
    items[existingIndex] = item;
  } else {
    items.unshift(item);
  }
  
  // Keep only the last 20 items
  saveContinueWatching(items.slice(0, 20));
}

export function getFavorites(): string[] {
  if (typeof window === "undefined") return [];
  
  try {
    const data = localStorage.getItem(STORAGE_KEYS.FAVORITES);
    return data ? JSON.parse(data) : [];
  } catch (error) {
    console.error("Error loading favorites:", error);
    return [];
  }
}

export function saveFavorites(ids: string[]): void {
  if (typeof window === "undefined") return;
  
  try {
    localStorage.setItem(STORAGE_KEYS.FAVORITES, JSON.stringify(ids));
  } catch (error) {
    console.error("Error saving favorites:", error);
  }
}

export function toggleFavorite(id: string): boolean {
  const favorites = getFavorites();
  const index = favorites.indexOf(id);
  
  if (index >= 0) {
    favorites.splice(index, 1);
    saveFavorites(favorites);
    return false;
  } else {
    favorites.push(id);
    saveFavorites(favorites);
    return true;
  }
}
