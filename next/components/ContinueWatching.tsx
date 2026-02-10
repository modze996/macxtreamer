"use client";

import { useRef } from "react";
import { ChevronLeft, ChevronRight, Play } from "lucide-react";
import Image from "next/image";

interface ContinueWatchingItem {
  id: string;
  title: string;
  subtitle?: string;
  thumbnailUrl?: string;
  progress: number; // 0-100
  type: "channel" | "series" | "movie";
}

interface ContinueWatchingProps {
  items: ContinueWatchingItem[];
}

export default function ContinueWatching({ items }: ContinueWatchingProps) {
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  const scroll = (direction: "left" | "right") => {
    if (scrollContainerRef.current) {
      const scrollAmount = direction === "left" ? -400 : 400;
      scrollContainerRef.current.scrollBy({
        left: scrollAmount,
        behavior: "smooth",
      });
    }
  };

  return (
    <div className="mb-10">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-2xl font-semibold text-white">Weiterschauen</h2>
        
        <div className="flex gap-2">
          <button
            onClick={() => scroll("left")}
            className="p-2 rounded-lg bg-gray-800/50 hover:bg-gray-700/50 text-gray-400 hover:text-white transition-all"
            aria-label="Nach links scrollen"
          >
            <ChevronLeft size={20} />
          </button>
          <button
            onClick={() => scroll("right")}
            className="p-2 rounded-lg bg-gray-800/50 hover:bg-gray-700/50 text-gray-400 hover:text-white transition-all"
            aria-label="Nach rechts scrollen"
          >
            <ChevronRight size={20} />
          </button>
        </div>
      </div>

      <div
        ref={scrollContainerRef}
        className="flex gap-4 overflow-x-auto scrollbar-hide scroll-smooth pb-2"
        style={{ scrollbarWidth: "none", msOverflowStyle: "none" }}
      >
        {items.map((item) => (
          <div
            key={item.id}
            className="flex-shrink-0 w-96 group cursor-pointer"
          >
            <div className="relative rounded-xl overflow-hidden bg-gray-800 h-56 transition-all duration-300 hover:scale-105 hover:shadow-xl hover:shadow-black/50">
              {/* Thumbnail or Placeholder */}
              {item.thumbnailUrl ? (
                <Image
                  src={item.thumbnailUrl}
                  alt={item.title}
                  fill
                  className="object-cover"
                />
              ) : (
                <div className="w-full h-full flex items-center justify-center bg-gradient-to-br from-gray-800 to-gray-900">
                  <div className="text-center px-4">
                    <div className="text-3xl mb-2">📺</div>
                    <p className="text-sm text-gray-400 font-medium line-clamp-2">
                      {item.title}
                    </p>
                  </div>
                </div>
              )}

              {/* Play Button Overlay */}
              <div className="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity duration-300 flex items-center justify-center">
                <div className="w-16 h-16 rounded-full bg-yellow-500 flex items-center justify-center shadow-lg">
                  <Play className="text-black fill-black ml-1" size={28} />
                </div>
              </div>

              {/* Progress Indicator */}
              <div className="absolute bottom-0 left-0 right-0 h-1 bg-gray-700">
                <div
                  className="h-full bg-yellow-500 transition-all"
                  style={{ width: `${item.progress}%` }}
                />
              </div>

              {/* Progress Circle Badge */}
              <div className="absolute top-3 right-3 w-12 h-12">
                <svg className="transform -rotate-90 w-12 h-12">
                  <circle
                    cx="24"
                    cy="24"
                    r="20"
                    stroke="rgba(255,255,255,0.2)"
                    strokeWidth="3"
                    fill="none"
                  />
                  <circle
                    cx="24"
                    cy="24"
                    r="20"
                    stroke="#EAB308"
                    strokeWidth="3"
                    fill="none"
                    strokeDasharray={`${2 * Math.PI * 20}`}
                    strokeDashoffset={`${2 * Math.PI * 20 * (1 - item.progress / 100)}`}
                    className="transition-all duration-300"
                  />
                </svg>
                <div className="absolute inset-0 flex items-center justify-center">
                  <span className="text-xs font-bold text-white">
                    {Math.round(item.progress)}%
                  </span>
                </div>
              </div>

              {/* Type Badge */}
              <div className="absolute top-3 left-3 bg-black/60 backdrop-blur-sm px-2.5 py-1 rounded-md">
                <span className="text-xs font-medium text-white uppercase">
                  {item.type === "channel" ? "Live" : item.type === "series" ? "Serie" : "Film"}
                </span>
              </div>
            </div>

            {/* Title and Subtitle */}
            <div className="mt-3">
              <h3 className="text-base font-medium text-white line-clamp-1">
                {item.title}
              </h3>
              {item.subtitle && (
                <p className="text-sm text-gray-400 mt-1 line-clamp-1">
                  {item.subtitle}
                </p>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
