"use client";

import { useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import Image from "next/image";
import { ArrowLeft, Play, Star, Calendar, Clock } from "lucide-react";
import Link from "next/link";

interface SeriesInfo {
  name: string;
  plot?: string;
  cover?: string;
  rating?: string;
  year?: string;
  genre?: string;
  director?: string;
  cast?: string;
  releaseDate?: string;
  episodeRunTime?: string;
}

interface Episode {
  episodeId: string;
  name: string;
  season: string;
  episodeNum: string;
  cover?: string;
  containerExtension?: string;
  streamUrl?: string | null;
}

interface SeasonData {
  [season: string]: Episode[];
}

export default function SeriesDetailPage() {
  const params = useParams();
  const router = useRouter();
  const seriesId = params.seriesId as string;

  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [seriesInfo, setSeriesInfo] = useState<SeriesInfo | null>(null);
  const [seasons, setSeasons] = useState<SeasonData>({});
  const [selectedSeason, setSelectedSeason] = useState<string>("1");

  useEffect(() => {
    const fetchSeriesData = async () => {
      try {
        const response = await fetch(`/api/episodes?series_id=${seriesId}`);
        
        if (!response.ok) {
          throw new Error("Failed to fetch series data");
        }

        const data = await response.json();
        
        // Extract series info
        if (data.info) {
          setSeriesInfo({
            name: data.info.name || "Unknown Series",
            plot: data.info.plot || data.info.description,
            cover: data.info.cover || data.info.movie_image,
            rating: data.info.rating_5based || data.info.rating,
            year: data.info.releaseDate?.substring(0, 4) || data.info.year,
            genre: data.info.genre,
            director: data.info.director,
            cast: data.info.cast,
            releaseDate: data.info.releaseDate,
            episodeRunTime: data.info.episode_run_time,
          });
        }

        // Group episodes by season
        const episodesBySeason: SeasonData = {};
        
        if (data.episodes && typeof data.episodes === "object") {
          for (const season in data.episodes) {
            episodesBySeason[season] = data.episodes[season].map((ep: any) => ({
              episodeId: ep.id || ep.episode_id || ep.stream_id || "",
              name: ep.title || ep.name || `Episode ${ep.episode_num || ""}`,
              season: season,
              episodeNum: ep.episode_num || "",
              cover: ep.info?.movie_image || ep.cover || data.info?.cover,
              containerExtension: ep.container_extension || "mp4",
              streamUrl: ep.stream_url || null,
            }));
          }
        }

        setSeasons(episodesBySeason);
        
        // Set first season as selected
        const firstSeason = Object.keys(episodesBySeason)[0];
        if (firstSeason) {
          setSelectedSeason(firstSeason);
        }

        setLoading(false);
      } catch (err) {
        console.error("Error fetching series:", err);
        setError(err instanceof Error ? err.message : "Failed to load series");
        setLoading(false);
      }
    };

    if (seriesId) {
      fetchSeriesData();
    }
  }, [seriesId]);

  if (loading) {
    return (
      <div className="min-h-screen bg-[#050505] flex items-center justify-center">
        <div className="text-center">
          <div className="inline-block animate-spin rounded-full h-12 w-12 border-4 border-yellow-500 border-t-transparent"></div>
          <p className="mt-4 text-gray-400">Lade Serie...</p>
        </div>
      </div>
    );
  }

  if (error || !seriesInfo) {
    return (
      <div className="min-h-screen bg-[#050505] flex items-center justify-center p-4">
        <div className="max-w-md w-full">
          <div className="bg-red-900/20 border border-red-700 rounded-lg p-6">
            <p className="text-red-200 mb-4">{error || "Serie nicht gefunden"}</p>
            <button
              onClick={() => router.back()}
              className="flex items-center gap-2 text-white hover:text-yellow-500 transition-colors"
            >
              <ArrowLeft size={20} />
              Zurück
            </button>
          </div>
        </div>
      </div>
    );
  }

  const seasonNumbers = Object.keys(seasons).sort((a, b) => Number(a) - Number(b));
  const currentEpisodes = seasons[selectedSeason] || [];

  return (
    <div className="min-h-screen bg-[#050505]">
      {/* Header with Back Button */}
      <div className="sticky top-0 z-10 bg-[#050505]/95 backdrop-blur-sm border-b border-gray-800">
        <div className="max-w-7xl mx-auto px-4 py-4">
          <Link
            href="/"
            className="inline-flex items-center gap-2 text-gray-400 hover:text-white transition-colors"
          >
            <ArrowLeft size={20} />
            <span>Zurück zur Startseite</span>
          </Link>
        </div>
      </div>

      {/* Hero Section */}
      <div className="relative h-[50vh] md:h-[60vh]">
        {seriesInfo.cover && (
          <div className="absolute inset-0">
            <Image
              src={seriesInfo.cover}
              alt={seriesInfo.name}
              fill
              className="object-cover"
              priority
            />
            <div className="absolute inset-0 bg-gradient-to-t from-[#050505] via-[#050505]/60 to-transparent" />
          </div>
        )}
        
        <div className="absolute bottom-0 left-0 right-0 p-4 md:p-8">
          <div className="max-w-7xl mx-auto">
            <h1 className="text-4xl md:text-6xl font-bold text-white mb-4">
              {seriesInfo.name}
            </h1>
            
            <div className="flex flex-wrap items-center gap-4 text-sm md:text-base">
              {seriesInfo.rating && (
                <div className="flex items-center gap-2 bg-yellow-500/20 px-3 py-1 rounded-full">
                  <Star className="fill-yellow-500 text-yellow-500" size={16} />
                  <span className="text-yellow-500 font-semibold">
                    {seriesInfo.rating}
                  </span>
                </div>
              )}
              
              {seriesInfo.year && (
                <div className="flex items-center gap-2 text-gray-300">
                  <Calendar size={16} />
                  <span>{seriesInfo.year}</span>
                </div>
              )}
              
              {seriesInfo.episodeRunTime && (
                <div className="flex items-center gap-2 text-gray-300">
                  <Clock size={16} />
                  <span>{seriesInfo.episodeRunTime} Min</span>
                </div>
              )}
              
              {seriesInfo.genre && (
                <span className="text-gray-300">{seriesInfo.genre}</span>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Content */}
      <div className="max-w-7xl mx-auto px-4 py-8">
        {/* Plot */}
        {seriesInfo.plot && (
          <div className="mb-8">
            <h2 className="text-2xl font-semibold text-white mb-4">Handlung</h2>
            <p className="text-gray-300 text-lg leading-relaxed max-w-4xl">
              {seriesInfo.plot}
            </p>
          </div>
        )}

        {/* Cast & Director */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-8">
          {seriesInfo.director && (
            <div>
              <h3 className="text-lg font-semibold text-white mb-2">Regie</h3>
              <p className="text-gray-400">{seriesInfo.director}</p>
            </div>
          )}
          
          {seriesInfo.cast && (
            <div>
              <h3 className="text-lg font-semibold text-white mb-2">Besetzung</h3>
              <p className="text-gray-400">{seriesInfo.cast}</p>
            </div>
          )}
        </div>

        {/* Seasons & Episodes */}
        <div className="mt-12">
          <h2 className="text-2xl font-semibold text-white mb-6">Episoden</h2>
          
          {/* Season Selector */}
          {seasonNumbers.length > 0 && (
            <div className="flex gap-2 mb-6 overflow-x-auto pb-2">
              {seasonNumbers.map((season) => (
                <button
                  key={season}
                  onClick={() => setSelectedSeason(season)}
                  className={`px-6 py-2 rounded-lg font-medium transition-all whitespace-nowrap ${
                    selectedSeason === season
                      ? "bg-yellow-500 text-black"
                      : "bg-gray-800 text-gray-400 hover:bg-gray-700 hover:text-white"
                  }`}
                >
                  Staffel {season}
                </button>
              ))}
            </div>
          )}

          {/* Episodes Grid */}
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {currentEpisodes.map((episode, index) => (
              <div
                key={episode.episodeId}
                className="bg-gray-900/50 rounded-lg overflow-hidden border border-gray-800 hover:border-yellow-500/50 transition-all group cursor-pointer"
              >
                <div className="relative aspect-video bg-gray-800">
                  {episode.cover ? (
                    <Image
                      src={episode.cover}
                      alt={episode.name}
                      fill
                      className="object-cover"
                    />
                  ) : (
                    <div className="w-full h-full flex items-center justify-center text-gray-600">
                      <Play size={48} />
                    </div>
                  )}
                  
                  <div className="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
                    <div className="w-16 h-16 rounded-full bg-yellow-500 flex items-center justify-center">
                      <Play className="text-black fill-black ml-1" size={24} />
                    </div>
                  </div>
                  
                  {episode.episodeNum && (
                    <div className="absolute top-2 left-2 bg-black/80 px-2 py-1 rounded text-xs font-semibold text-white">
                      E{episode.episodeNum}
                    </div>
                  )}
                </div>
                
                <div className="p-4">
                  <h3 className="text-white font-medium line-clamp-2">
                    {episode.name}
                  </h3>
                </div>
              </div>
            ))}
          </div>

          {currentEpisodes.length === 0 && (
            <div className="text-center py-12">
              <p className="text-gray-400">
                Keine Episoden für Staffel {selectedSeason} gefunden
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
