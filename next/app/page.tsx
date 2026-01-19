"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { Play, Grid3x3 } from "lucide-react";

interface Category {
  id: string;
  name: string;
}

interface ConfigStatus {
  configured: boolean;
  valid?: boolean;
  address?: string;
  username?: string;
  error?: string;
}

export default function Home() {
  const [configStatus, setConfigStatus] = useState<ConfigStatus | null>(null);
  const [liveCategories, setLiveCategories] = useState<Category[]>([]);
  const [vodCategories, setVodCategories] = useState<Category[]>([]);
  const [seriesCategories, setSeriesCategories] = useState<Category[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchData = async () => {
      try {
        // Check config
        const configRes = await fetch("/api/config");
        const configData = await configRes.json();
        setConfigStatus(configData);

        if (!configData.configured) {
          setError(
            "Configuration not found. Please configure macxtreamer first."
          );
          setLoading(false);
          return;
        }

        if (!configData.valid) {
          setError(
            "Configuration is invalid. Please check your credentials."
          );
          setLoading(false);
          return;
        }

        // Fetch categories
        const [liveRes, vodRes, seriesRes] = await Promise.all([
          fetch("/api/categories?action=get_live_categories"),
          fetch("/api/categories?action=get_vod_categories"),
          fetch("/api/categories?action=get_series_categories"),
        ]);

        if (liveRes.ok) {
          setLiveCategories(await liveRes.json());
        }
        if (vodRes.ok) {
          setVodCategories(await vodRes.json());
        }
        if (seriesRes.ok) {
          setSeriesCategories(await seriesRes.json());
        }
      } catch (err) {
        setError(
          err instanceof Error
            ? err.message
            : "Failed to load data"
        );
      } finally {
        setLoading(false);
      }
    };

    fetchData();
  }, []);

  if (loading) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="text-center">
          <div className="inline-block animate-spin rounded-full h-12 w-12 border-b-2 border-white"></div>
          <p className="mt-4">Loading...</p>
        </div>
      </div>
    );
  }

  return (
    <main className="min-h-screen bg-gradient-to-br from-gray-900 to-black">
      {/* Header */}
      <header className="bg-gray-800 border-b border-gray-700 sticky top-0 z-50">
        <div className="max-w-7xl mx-auto px-4 py-6">
          <div className="flex items-center justify-between">
            <div>
              <h1 className="text-3xl font-bold">MacXStreamer Web</h1>
              {configStatus?.address && (
                <p className="text-gray-400 text-sm mt-2">
                  Connected to: {configStatus.address}
                </p>
              )}
            </div>
            <Link
              href="/browser"
              className="flex items-center gap-2 bg-blue-600 hover:bg-blue-700 rounded-lg px-6 py-3 font-semibold transition"
            >
              <Grid3x3 size={20} />
              Browser View
            </Link>
          </div>
        </div>
      </header>

      {error ? (
        <div className="max-w-7xl mx-auto px-4 py-8">
          <div className="bg-red-900 border border-red-700 rounded-lg p-4">
            <p className="text-red-200">{error}</p>
          </div>
        </div>
      ) : (
        <div className="max-w-7xl mx-auto px-4 py-8">
          {/* Live Categories */}
          {liveCategories.length > 0 && (
            <section className="mb-12">
              <h2 className="text-2xl font-bold mb-6">📺 Live TV</h2>
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                {liveCategories.map((cat) => (
                  <Link
                    key={cat.id}
                    href={`/live/${cat.id}`}
                    className="bg-gray-800 hover:bg-gray-700 rounded-lg p-4 transition cursor-pointer border border-gray-700"
                  >
                    <h3 className="font-semibold text-lg">{cat.name}</h3>
                    <p className="text-gray-400 text-sm mt-2">
                      View channels →
                    </p>
                  </Link>
                ))}
              </div>
            </section>
          )}

          {/* VOD Categories */}
          {vodCategories.length > 0 && (
            <section className="mb-12">
              <h2 className="text-2xl font-bold mb-6">🎬 Movies</h2>
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                {vodCategories.map((cat) => (
                  <Link
                    key={cat.id}
                    href={`/vod/${cat.id}`}
                    className="bg-gray-800 hover:bg-gray-700 rounded-lg p-4 transition cursor-pointer border border-gray-700"
                  >
                    <h3 className="font-semibold text-lg">{cat.name}</h3>
                    <p className="text-gray-400 text-sm mt-2">
                      View movies →
                    </p>
                  </Link>
                ))}
              </div>
            </section>
          )}

          {/* Series Categories */}
          {seriesCategories.length > 0 && (
            <section className="mb-12">
              <h2 className="text-2xl font-bold mb-6">📺 Series</h2>
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                {seriesCategories.map((cat) => (
                  <Link
                    key={cat.id}
                    href={`/series/${cat.id}`}
                    className="bg-gray-800 hover:bg-gray-700 rounded-lg p-4 transition cursor-pointer border border-gray-700"
                  >
                    <h3 className="font-semibold text-lg">{cat.name}</h3>
                    <p className="text-gray-400 text-sm mt-2">
                      View series →
                    </p>
                  </Link>
                ))}
              </div>
            </section>
          )}

          {liveCategories.length === 0 &&
            vodCategories.length === 0 &&
            seriesCategories.length === 0 && (
              <div className="bg-gray-800 border border-gray-700 rounded-lg p-8 text-center">
                <p className="text-gray-400">No categories found</p>
              </div>
            )}
        </div>
      )}
    </main>
  );
}
