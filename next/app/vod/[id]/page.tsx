"use client";

import { useParams, useRouter } from "next/navigation";
import { useEffect, useState } from "react";

interface Item {
  id: string;
  name: string;
  cover?: string;
  plot?: string;
  streamUrl?: string;
  year?: string;
  rating?: number;
  genre?: string;
  director?: string;
  cast?: string;
}

export default function VodCategory() {
  const params = useParams();
  const router = useRouter();
  const categoryId = params.id as string;
  const [items, setItems] = useState<Item[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchItems = async () => {
      try {
        const res = await fetch(
          `/api/items?action=get_vod_streams&category_id=${categoryId}`
        );

        if (!res.ok) {
          throw new Error(`Failed to fetch items: ${res.status}`);
        }

        setItems(await res.json());
      } catch (err) {
        setError(
          err instanceof Error
            ? err.message
            : "Failed to load items"
        );
      } finally {
        setLoading(false);
      }
    };

    fetchItems();
  }, [categoryId]);

  return (
    <main className="min-h-screen bg-gradient-to-br from-gray-900 to-black">
      {/* Header */}
      <header className="bg-gray-800 border-b border-gray-700 sticky top-0 z-50">
        <div className="max-w-7xl mx-auto px-4 py-4 flex items-center gap-4">
          <button
            onClick={() => router.back()}
            className="bg-gray-700 hover:bg-gray-600 px-4 py-2 rounded"
          >
            ← Back
          </button>
          <h1 className="text-2xl font-bold">Movies</h1>
        </div>
      </header>

      <div className="max-w-7xl mx-auto px-4 py-8">
        {loading ? (
          <div className="text-center">
            <div className="inline-block animate-spin rounded-full h-12 w-12 border-b-2 border-white"></div>
          </div>
        ) : error ? (
          <div className="bg-red-900 border border-red-700 rounded-lg p-4">
            <p className="text-red-200">{error}</p>
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            {items.map((item) => (
              <div
                key={item.id}
                className="bg-gray-800 rounded-lg overflow-hidden border border-gray-700 hover:border-blue-500 transition group"
              >
                {item.cover ? (
                  <div className="relative aspect-video overflow-hidden bg-gray-700">
                    <img
                      src={item.cover}
                      alt={item.name}
                      className="w-full h-full object-cover group-hover:scale-105 transition"
                      onError={(e) => {
                        e.currentTarget.src =
                          "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='100' height='100'%3E%3Crect fill='%23374151' width='100' height='100'/%3E%3Ctext x='50%' y='50%' font-size='12' fill='%239CA3AF' text-anchor='middle' dy='.3em'%3ENo Image%3C/text%3E%3C/svg%3E";
                      }}
                    />
                  </div>
                ) : (
                  <div className="aspect-video bg-gray-700 flex items-center justify-center">
                    <span className="text-gray-400">No Image</span>
                  </div>
                )}
                <div className="p-3">
                  <h3 className="font-semibold text-sm truncate">
                    {item.name}
                  </h3>
                  {item.year && (
                    <p className="text-gray-400 text-xs">{item.year}</p>
                  )}
                  {item.rating && (
                    <p className="text-yellow-400 text-xs">
                      ★ {item.rating.toFixed(1)}
                    </p>
                  )}
                  <button
                    onClick={() => {
                      if (item.streamUrl) {
                        window.open(item.streamUrl, "_blank");
                      }
                    }}
                    className="mt-2 w-full bg-blue-600 hover:bg-blue-700 px-3 py-2 rounded text-sm transition"
                    disabled={!item.streamUrl}
                  >
                    {item.streamUrl ? "Play" : "No Stream"}
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </main>
  );
}
