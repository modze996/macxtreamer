"use client";

import { useEffect, useState } from "react";
import Sidebar from "@/components/Sidebar";
import TopBar from "@/components/TopBar";
import ContinueWatching from "@/components/ContinueWatching";
import HorizontalRow from "@/components/HorizontalRow";
import TvGuide from "@/components/TvGuide";
import {
  HorizontalRowSkeleton,
  ContinueWatchingSkeleton,
  TvGuideSkeleton,
} from "@/components/LoadingSkeletons";
import {
  fetchCategories,
  fetchItems,
  convertStreamToContentItem,
  getContinueWatching,
  type Category,
  type ContentItem,
  type ContinueWatchingItem,
} from "@/lib/api";

interface ConfigStatus {
  configured: boolean;
  valid?: boolean;
  address?: string;
  username?: string;
  error?: string;
}

export default function Home() {
  const [configStatus, setConfigStatus] = useState<ConfigStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  
  // Real data states
  const [seriesCategories, setSeriesCategories] = useState<Category[]>([]);
  const [vodCategories, setVodCategories] = useState<Category[]>([]);
  const [popularSeries, setPopularSeries] = useState<ContentItem[]>([]);
  const [popularMovies, setPopularMovies] = useState<ContentItem[]>([]);
  const [favoriteSeries, setFavoriteSeries] = useState<ContentItem[]>([]);
  const [continueWatchingItems, setContinueWatchingItems] = useState<ContinueWatchingItem[]>([]);

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

        // Load continue watching from localStorage
        const continueWatching = getContinueWatching();
        setContinueWatchingItems(continueWatching);

        // Fetch categories
        const [seriesCats, vodCats] = await Promise.all([
          fetchCategories("series"),
          fetchCategories("vod"),
        ]);

        setSeriesCategories(seriesCats);
        setVodCategories(vodCats);

        // Fetch popular content from first categories
        if (seriesCats.length > 0) {
          const seriesItems = await fetchItems("series", seriesCats[0].id);
          const converted = seriesItems
            .slice(0, 6)
            .map((item, index) => convertStreamToContentItem(item, index + 1));
          setPopularSeries(converted);
          
          // Use some for favorites too (in real app, filter by user's favorites)
          setFavoriteSeries(converted.slice(0, 5));
        }

        if (vodCats.length > 0) {
          const vodItems = await fetchItems("vod", vodCats[0].id);
          const converted = vodItems
            .slice(0, 6)
            .map((item, index) => convertStreamToContentItem(item, index + 1));
          setPopularMovies(converted);
        }

        setLoading(false);
      } catch (err) {
        setError(
          err instanceof Error
            ? err.message
            : "Failed to load data"
        );
        setLoading(false);
      }
    };

    fetchData();
  }, []);

  // Mock data for demo purposes
  const xtreamAccounts = [
    { id: "1", name: configStatus?.address ? "Main IPTV" : "IPTV Account" },
  ];

  const tvGuideChannels = [
    {
      id: "1",
      name: "Sky Bundesliga 1 HD",
      category: "bundesliga",
      programs: [
        { id: "1", title: "Bundesliga Konferenz", startTime: "15:30", endTime: "17:30", duration: 120 },
        { id: "2", title: "Highlights & Analysen", startTime: "17:30", endTime: "18:30", duration: 60 },
      ],
    },
    {
      id: "2",
      name: "Sky Bundesliga 2 HD",
      category: "bundesliga",
      programs: [
        { id: "3", title: "Bayern München - Dortmund", startTime: "15:30", endTime: "17:15", duration: 105 },
        { id: "4", title: "Vorschau", startTime: "17:15", endTime: "17:45", duration: 30 },
      ],
    },
    {
      id: "3",
      name: "Sky Sport 2. Liga HD",
      category: "2bundesliga",
      programs: [
        { id: "5", title: "2. Bundesliga Konferenz", startTime: "13:00", endTime: "15:00", duration: 120 },
        { id: "6", title: "Highlights", startTime: "15:00", endTime: "16:00", duration: 60 },
      ],
    },
    {
      id: "4",
      name: "DAZN 1 HD",
      category: "dazn",
      programs: [
        { id: "7", title: "Champions League Live", startTime: "21:00", endTime: "23:00", duration: 120 },
      ],
    },
    {
      id: "5",
      name: "DAZN 2 HD",
      category: "dazn",
      programs: [
        { id: "8", title: "UEFA Europa League", startTime: "18:45", endTime: "20:45", duration: 120 },
        { id: "9", title: "Highlights", startTime: "20:45", endTime: "21:30", duration: 45 },
      ],
    },
  ];

  if (loading) {
    return (
      <div className="flex min-h-screen bg-[#050505]">
        <Sidebar accounts={xtreamAccounts} />
        
        <main className="flex-1 lg:ml-64 p-4 md:p-6 lg:p-8 pt-16 lg:pt-8">
          <TopBar 
            title="Startseite" 
            onSearch={(query) => console.log("Search:", query)}
          />
          
          {/* Loading Skeletons */}
          <ContinueWatchingSkeleton />
          <HorizontalRowSkeleton type="poster" />
          <HorizontalRowSkeleton type="poster" />
          <HorizontalRowSkeleton type="poster" />
          <TvGuideSkeleton />
        </main>
      </div>
    );
  }

  if (error) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-[#050505]">
        <div className="max-w-md w-full mx-4">
          <div className="bg-red-900/20 border border-red-700 rounded-lg p-6">
            <p className="text-red-200">{error}</p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex min-h-screen bg-[#050505]">
      {/* Sidebar */}
      <Sidebar accounts={xtreamAccounts} />

      {/* Main Content */}
      <main className="flex-1 lg:ml-64 p-4 md:p-6 lg:p-8 pt-16 lg:pt-8">
        {/* Top Bar */}
        <TopBar 
          title="Startseite" 
          onSearch={(query) => console.log("Search:", query)}
        />

        {/* Continue Watching */}
        {continueWatchingItems.length > 0 && (
          <ContinueWatching items={continueWatchingItems} />
        )}

        {/* Favorite Series */}
        {favoriteSeries.length > 0 && (
          <HorizontalRow 
            title="Lieblingsserien" 
            items={favoriteSeries}
            type="poster"
            contentType="series"
          />
        )}

        {/* Popular Series */}
        {popularSeries.length > 0 && (
          <HorizontalRow 
            title="Beliebte Serien" 
            items={popularSeries}
            type="poster"
            contentType="series"
          />
        )}

        {/* Popular Movies */}
        {popularMovies.length > 0 && (
          <HorizontalRow 
            title="Beliebte Filme" 
            items={popularMovies}
            type="poster"
            contentType="vod"
          />
        )}

        {/* Show message if no content */}
        {continueWatchingItems.length === 0 && 
         popularSeries.length === 0 && 
         popularMovies.length === 0 && 
         favoriteSeries.length === 0 && (
          <div className="flex items-center justify-center py-20">
            <div className="text-center">
              <div className="text-6xl mb-4">📺</div>
              <h3 className="text-xl font-semibold text-white mb-2">
                Willkommen bei nextreamer
              </h3>
              <p className="text-gray-400">
                Laden Sie Inhalte von Ihrem IPTV-Anbieter...
              </p>
            </div>
          </div>
        )}

        {/* TV Guide */}
        <TvGuide channels={tvGuideChannels} loading={false} />

        {/* Footer Status */}
        <div className="flex justify-center mt-8 mb-4">
          <div className="bg-gray-800/50 border border-gray-700 rounded-full px-4 py-2 text-sm text-gray-400">
            EPG wurden geladen
          </div>
        </div>
      </main>
    </div>
  );
}
