"use client";

import { useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import Image from "next/image";
import { ArrowLeft, Play, Star, Calendar, Clock, Globe } from "lucide-react";
import Link from "next/link";
import Sidebar from "@/components/Sidebar";
import TopBar from "@/components/TopBar";

interface MovieInfo {
  streamId: string;
  name: string;
  plot?: string;
  cover?: string;
  rating?: string;
  year?: string;
  genre?: string;
  director?: string;
  cast?: string;
  releaseDate?: string;
  duration?: string;
  country?: string;
  videoCodec?: string;
  audioLanguages?: string;
  containerExtension?: string;
}

export default function VODDetailPage() {
  const params = useParams();
  const router = useRouter();
  const vodId = params.vodId as string;

  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [movieInfo, setMovieInfo] = useState<MovieInfo | null>(null);

  useEffect(() => {
    const fetchMovieData = async () => {
      try {
        const response = await fetch(`/api/items?action=get_vod_info&vod_id=${vodId}`);
        
        if (!response.ok) {
          throw new Error("Failed to fetch movie data");
        }

        const data = await response.json();
        
        // The API might return the data directly or in a 'movie' or 'info' field
        const movieData = data.info || data.movie || data;
        
        setMovieInfo({
          streamId: vodId,
          name: movieData.name || movieData.title || "Unknown Movie",
          plot: movieData.plot || movieData.description,
          cover: movieData.movie_image || movieData.cover || movieData.stream_icon,
          rating: movieData.rating_5based || movieData.rating,
          year: movieData.releasedate?.substring(0, 4) || movieData.year,
          genre: movieData.genre,
          director: movieData.director,
          cast: movieData.cast,
          releaseDate: movieData.releasedate,
          duration: movieData.duration || movieData.duration_secs,
          country: movieData.country,
          videoCodec: movieData.video?.codec_name,
          audioLanguages: movieData.audio?.language,
          containerExtension: movieData.container_extension || "mp4",
        });

        setLoading(false);
      } catch (err) {
        console.error("Error fetching movie:", err);
        setError(err instanceof Error ? err.message : "Failed to load movie");
        setLoading(false);
      }
    };

    if (vodId) {
      fetchMovieData();
    }
  }, [vodId]);

  if (loading) {
    return (
      <div className="min-h-screen bg-[#050505] flex items-center justify-center">
        <div className="text-center">
          <div className="inline-block animate-spin rounded-full h-12 w-12 border-4 border-yellow-500 border-t-transparent"></div>
          <p className="mt-4 text-gray-400">Lade Film...</p>
        </div>
      </div>
    );
  }

  if (error || !movieInfo) {
    return (
      <div className="min-h-screen bg-[#050505] flex items-center justify-center p-4">
        <div className="max-w-md w-full">
          <div className="bg-red-900/20 border border-red-700 rounded-lg p-6">
            <p className="text-red-200 mb-4">{error || "Film nicht gefunden"}</p>
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

  const formatDuration = (duration?: string) => {
    if (!duration) return null;
    const minutes = Math.floor(parseInt(duration) / 60);
    return `${minutes} Min`;
  };

  return (
    <div className="flex min-h-screen bg-[#050505]">
      <Sidebar />
      
      <div className="flex-1 flex flex-col lg:ml-64">
        <TopBar />
        
        <main className="flex-1 overflow-y-auto">
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

          {/* Hero Section with Poster and Info Layout */}
          <div className="relative">
            <div className="max-w-7xl mx-auto px-4 py-8">
              <div className="grid grid-cols-1 md:grid-cols-4 gap-8">
            {/* Poster/Cover on Left */}
            <div className="md:col-span-1">
              <div className="relative rounded-lg overflow-hidden bg-gray-800 shadow-2xl sticky top-24">
                {movieInfo.cover ? (
                  <div className="relative w-full aspect-[2/3]" >
                    <Image
                      src={movieInfo.cover}
                      alt={movieInfo.name}
                      fill
                      className="object-cover rounded-lg"
                      priority
                    />
                  </div>
                ) : (
                  <div className="w-full aspect-[2/3] bg-gradient-to-br from-gray-700 to-gray-900 rounded-lg flex items-center justify-center text-6xl">
                    🎬
                  </div>
                )}
              </div>
            </div>

            {/* Info and Details on Right */}
            <div className="md:col-span-3">
              <h1 className="text-4xl md:text-5xl font-bold text-white mb-6">
                {movieInfo.name}
              </h1>
              
              <div className="flex flex-wrap items-center gap-4 text-sm md:text-base mb-8">
                {movieInfo.rating && (
                  <div className="flex items-center gap-2 bg-yellow-500/20 px-3 py-1 rounded-full">
                    <Star className="fill-yellow-500 text-yellow-500" size={16} />
                    <span className="text-yellow-500 font-semibold">
                      {movieInfo.rating}
                    </span>
                  </div>
                )}
                
                {movieInfo.year && (
                  <div className="flex items-center gap-2 text-gray-300">
                    <Calendar size={16} />
                    <span>{movieInfo.year}</span>
                  </div>
                )}
                
                {movieInfo.duration && (
                  <div className="flex items-center gap-2 text-gray-300">
                    <Clock size={16} />
                    <span>{formatDuration(movieInfo.duration)}</span>
                  </div>
                )}
                
                {movieInfo.country && (
                  <div className="flex items-center gap-2 text-gray-300">
                    <Globe size={16} />
                    <span>{movieInfo.country}</span>
                  </div>
                )}
                
                {movieInfo.genre && (
                  <span className="text-gray-300">{movieInfo.genre}</span>
                )}
              </div>

              {/* Play Button */}
              <button className="flex items-center gap-3 bg-yellow-500 hover:bg-yellow-400 text-black font-semibold px-8 py-4 rounded-lg transition-all hover:scale-105 shadow-lg mb-8">
                <Play className="fill-black" size={24} />
                <span className="text-lg">Jetzt abspielen</span>
              </button>

              {/* Plot Section */}
              {movieInfo.plot && (
                <div>
                  <h2 className="text-xl font-semibold text-white mb-3">Handlung</h2>
                  <p className="text-gray-300 text-base leading-relaxed line-clamp-4">
                    {movieInfo.plot}
                  </p>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Details Section */}
      <div className="max-w-7xl mx-auto px-4 py-12 border-t border-gray-800">
        {/* Details Grid */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 mb-8">
          {movieInfo.director && (
            <div>
              <h3 className="text-lg font-semibold text-white mb-2">Regie</h3>
              <p className="text-gray-400">{movieInfo.director}</p>
            </div>
          )}
          
          {movieInfo.cast && (
            <div className="md:col-span-2">
              <h3 className="text-lg font-semibold text-white mb-2">Besetzung</h3>
              <p className="text-gray-400">{movieInfo.cast}</p>
            </div>
          )}

          {movieInfo.audioLanguages && (
            <div>
              <h3 className="text-lg font-semibold text-white mb-2">Sprache</h3>
              <p className="text-gray-400">{movieInfo.audioLanguages}</p>
            </div>
          )}

          {movieInfo.videoCodec && (
            <div>
              <h3 className="text-lg font-semibold text-white mb-2">Video Codec</h3>
              <p className="text-gray-400">{movieInfo.videoCodec}</p>
            </div>
          )}
        </div>

        {/* Technical Info */}
        {(movieInfo.containerExtension || movieInfo.releaseDate) && (
          <div className="bg-gray-900/50 rounded-lg p-6 border border-gray-800">
            <h3 className="text-lg font-semibold text-white mb-4">
              Technische Details
            </h3>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
              {movieInfo.containerExtension && (
                <div>
                  <p className="text-gray-500 mb-1">Format</p>
                  <p className="text-white font-medium uppercase">
                    {movieInfo.containerExtension}
                  </p>
                </div>
              )}
              {movieInfo.releaseDate && (
                <div>
                  <p className="text-gray-500 mb-1">Erscheinungsdatum</p>
                  <p className="text-white font-medium">
                    {new Date(movieInfo.releaseDate).toLocaleDateString('de-DE')}
                  </p>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
        </main>
      </div>
    </div>
  );
}
