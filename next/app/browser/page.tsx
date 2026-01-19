"use client";

import { useEffect, useState } from "react";
import Image from "next/image";
import { Menu, X } from "lucide-react";

interface Category {
  id: string;
  name: string;
}

interface Item {
  id: string;
  name: string;
  image?: string;
}

interface ConfigStatus {
  configured: boolean;
  valid?: boolean;
  address?: string;
}

type ContentType = "live" | "vod" | "series";

export default function Browser() {
  const [configStatus, setConfigStatus] = useState<ConfigStatus | null>(null);
  const [contentType, setContentType] = useState<ContentType>("live");
  const [categories, setCategories] = useState<Category[]>([]);
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const [items, setItems] = useState<Item[]>([]);
  const [loading, setLoading] = useState(true);
  const [itemsLoading, setItemsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(true);
  const [filterText, setFilterText] = useState("");

  const actionMap = {
    live: "get_live_categories",
    vod: "get_vod_categories",
    series: "get_series_categories",
  };

  // Fetch categories when content type changes
  useEffect(() => {
    const fetchCategories = async () => {
      try {
        setLoading(true);
        setItems([]);
        setSelectedCategory(null);
        setFilterText("");

        const res = await fetch(
          `/api/categories?action=${actionMap[contentType]}`
        );
        if (res.ok) {
          const data = await res.json();
          setCategories(data);
          if (data.length > 0) {
            setSelectedCategory(data[0].id);
          }
        }
      } catch (err) {
        setError(
          err instanceof Error ? err.message : "Failed to load categories"
        );
      } finally {
        setLoading(false);
      }
    };

    fetchCategories();
  }, [contentType]);

  // Fetch items when category changes
  useEffect(() => {
    const fetchItems = async () => {
      if (!selectedCategory) return;

      try {
        setItemsLoading(true);
        const res = await fetch(
          `/api/items?cat_id=${selectedCategory}&action=get_${contentType}_streams`
        );
        if (res.ok) {
          const data = await res.json();
          setItems(data);
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load items");
      } finally {
        setItemsLoading(false);
      }
    };

    fetchItems();
  }, [selectedCategory, contentType]);

  // Check config on mount
  useEffect(() => {
    const checkConfig = async () => {
      try {
        const res = await fetch("/api/config");
        const data = await res.json();
        setConfigStatus(data);
        if (!data.configured || !data.valid) {
          setError("Configuration not found or invalid");
        }
      } catch (err) {
        setError("Failed to check configuration");
      }
    };

    checkConfig();
  }, []);

  const currentCategory = categories.find((c) => c.id === selectedCategory);
  const filteredItems = items.filter((item) =>
    item.name.toLowerCase().includes(filterText.toLowerCase())
  );

  const getCategoryLabel = (type: ContentType) => {
    switch (type) {
      case "live":
        return "Live TV";
      case "vod":
        return "Movies";
      case "series":
        return "Series";
    }
  };

  if (!configStatus?.configured) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gray-900">
        <div className="bg-red-900 border border-red-700 rounded-lg p-8 max-w-md">
          <p className="text-red-200 text-center">
            Configuration not found. Please configure macxtreamer first.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-screen bg-gray-900 text-white">
      {/* Mobile overlay */}
      {drawerOpen && (
        <div
          className="fixed inset-0 bg-black bg-opacity-50 lg:hidden z-40"
          onClick={() => setDrawerOpen(false)}
        />
      )}

      {/* Sidebar/Drawer */}
      <aside
        className={`fixed lg:static inset-y-0 left-0 w-64 bg-gray-800 border-r border-gray-700 transition-transform duration-300 z-50 lg:z-auto ${
          drawerOpen ? "translate-x-0" : "-translate-x-full lg:translate-x-0"
        }`}
      >
        <div className="flex flex-col h-full">
          {/* Header */}
          <div className="p-6 border-b border-gray-700">
            <div className="flex items-center justify-between">
              <h1 className="text-xl font-bold">MacXStreamer</h1>
              <button
                onClick={() => setDrawerOpen(false)}
                className="lg:hidden text-gray-400 hover:text-white"
              >
                <X size={24} />
              </button>
            </div>
          </div>

          {/* Navigation */}
          <nav className="flex-1 p-4 space-y-2">
            {(
              [
                { type: "live" as ContentType, label: "📺 Live TV", icon: "📺" },
                { type: "vod" as ContentType, label: "🎬 Movies", icon: "🎬" },
                {
                  type: "series" as ContentType,
                  label: "📺 Series",
                  icon: "📺",
                },
              ] as const
            ).map(({ type, label }) => (
              <button
                key={type}
                onClick={() => {
                  setContentType(type);
                  setDrawerOpen(false);
                }}
                className={`w-full text-left px-4 py-3 rounded-lg transition ${
                  contentType === type
                    ? "bg-blue-600 text-white"
                    : "text-gray-300 hover:bg-gray-700"
                }`}
              >
                {label}
              </button>
            ))}
          </nav>

          {/* Categories list */}
          <div className="flex-1 border-t border-gray-700 overflow-hidden flex flex-col">
            <div className="p-4">
              <h3 className="text-xs uppercase font-semibold text-gray-400 mb-3">
                Categories
              </h3>
              <input
                type="text"
                placeholder="Filter..."
                value={filterText}
                onChange={(e) => setFilterText(e.target.value)}
                className="w-full px-3 py-2 bg-gray-700 text-white text-sm rounded border border-gray-600 focus:border-blue-500 focus:outline-none"
              />
            </div>

            <div className="flex-1 overflow-y-auto px-3 pb-4">
              {categories.map((cat) => (
                <button
                  key={cat.id}
                  onClick={() => {
                    setSelectedCategory(cat.id);
                    setFilterText("");
                  }}
                  className={`w-full text-left px-3 py-2 rounded text-sm transition mb-1 truncate ${
                    selectedCategory === cat.id
                      ? "bg-blue-600 text-white"
                      : "text-gray-300 hover:bg-gray-700"
                  }`}
                  title={cat.name}
                >
                  {cat.name}
                </button>
              ))}
            </div>
          </div>
        </div>
      </aside>

      {/* Main content */}
      <main className="flex-1 flex flex-col overflow-hidden">
        {/* Top bar */}
        <div className="bg-gray-800 border-b border-gray-700 px-6 py-4 flex items-center justify-between">
          <div className="flex items-center gap-4">
            <button
              onClick={() => setDrawerOpen(!drawerOpen)}
              className="lg:hidden text-gray-400 hover:text-white"
            >
              <Menu size={24} />
            </button>
            <div>
              <h2 className="text-2xl font-bold">{getCategoryLabel(contentType)}</h2>
              {currentCategory && (
                <p className="text-gray-400 text-sm mt-1">{currentCategory.name}</p>
              )}
            </div>
          </div>
          <div className="text-sm text-gray-400">
            {filteredItems.length} items
          </div>
        </div>

        {/* Content area */}
        <div className="flex-1 overflow-y-auto p-6">
          {error && (
            <div className="bg-red-900 border border-red-700 rounded-lg p-4 mb-6">
              <p className="text-red-200">{error}</p>
            </div>
          )}

          {itemsLoading && (
            <div className="flex items-center justify-center h-full">
              <div className="text-center">
                <div className="inline-block animate-spin rounded-full h-12 w-12 border-b-2 border-white"></div>
                <p className="mt-4 text-gray-400">Loading items...</p>
              </div>
            </div>
          )}

          {!itemsLoading && filteredItems.length === 0 && (
            <div className="flex items-center justify-center h-full">
              <p className="text-gray-400">No items found</p>
            </div>
          )}

          {!itemsLoading && filteredItems.length > 0 && (
            <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4 auto-rows-max">
              {filteredItems.map((item) => (
                <div
                  key={item.id}
                  className="group cursor-pointer bg-gray-800 rounded-lg overflow-hidden hover:shadow-lg hover:shadow-blue-500/50 transition transform hover:scale-105"
                >
                  {/* Image container */}
                  <div className="relative aspect-video bg-gray-700 overflow-hidden">
                    {item.image ? (
                      <Image
                        src={item.image}
                        alt={item.name}
                        fill
                        className="object-cover group-hover:brightness-110 transition"
                        onError={(e) => {
                          const target = e.target as HTMLImageElement;
                          target.style.display = "none";
                        }}
                      />
                    ) : (
                      <div className="w-full h-full flex items-center justify-center text-gray-500">
                        <span className="text-3xl">📺</span>
                      </div>
                    )}
                  </div>

                  {/* Title */}
                  <div className="p-2">
                    <h3 className="text-xs font-semibold line-clamp-2 group-hover:text-blue-400 transition">
                      {item.name}
                    </h3>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </main>
    </div>
  );
}
