"use client";

import { useRef } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import Image from "next/image";
import Link from "next/link";

interface ContentItem {
  id: string;
  title: string;
  subtitle?: string;
  coverUrl?: string;
  ranking?: number;
}

interface HorizontalRowProps {
  title: string;
  items: ContentItem[];
  type?: "poster" | "landscape";
  contentType?: "series" | "vod" | "live";
}

export default function HorizontalRow({ 
  title, 
  items, 
  type = "poster",
  contentType = "vod"
}: HorizontalRowProps) {
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

  const getItemHref = (id: string) => {
    if (contentType === "series") return `/series/detail/${id}`;
    if (contentType === "vod") return `/vod/detail/${id}`;
    if (contentType === "live") return `/live/${id}`;
    return "#";
  };

  return (
    <div className="mb-10">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-2xl font-semibold text-white">{title}</h2>
        
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
        className="flex gap-4 overflow-x-auto scrollbar-hide scroll-smooth"
        style={{ scrollbarWidth: "none", msOverflowStyle: "none" }}
      >
        {items.map((item, index) => (
          <Link
            key={item.id}
            href={getItemHref(item.id)}
            className="flex-shrink-0 group cursor-pointer block"
          >
            <div
              className={`relative rounded-lg overflow-hidden bg-gray-800 transition-all duration-300 hover:scale-105 hover:shadow-xl hover:shadow-black/50 ${
                type === "poster" ? "w-44 h-64" : "w-80 h-44"
              }`}
            >
              {/* Ranking Badge */}
              {item.ranking && (
                <div className="absolute top-2 left-2 z-10 bg-yellow-500 text-black font-bold text-sm px-2.5 py-1 rounded-md">
                  #{item.ranking}
                </div>
              )}

              {/* Cover Image or Placeholder */}
              {item.coverUrl ? (
                <Image
                  src={item.coverUrl}
                  alt={item.title}
                  width={type === "poster" ? 176 : 320}
                  height={type === "poster" ? 256 : 176}
                  className="h-full w-full object-cover rounded-lg"
                />
              ) : (
                <div className="w-full h-full flex items-center justify-center bg-gradient-to-br from-gray-800 to-gray-900">
                  <div className="text-center px-2">
                    <div className="text-2xl mb-3">🎬</div>
                    <p className="text-xs font-medium text-gray-400 line-clamp-2">
                      {item.title}
                    </p>
                  </div>
                </div>
              )}

              {/* Hover Overlay */}
              <div className="absolute inset-0 bg-gradient-to-t from-black/80 via-black/0 to-black/0 opacity-0 group-hover:opacity-100 transition-opacity duration-300" />
            </div>

            {/* Title and Subtitle */}
            <div className="mt-3">
              <h3 className="text-sm font-medium text-white line-clamp-1">
                {item.title}
              </h3>
              {item.subtitle && (
                <p className="text-xs text-gray-400 mt-1 line-clamp-1">
                  {item.subtitle}
                </p>
              )}
            </div>
          </Link>
        ))}
      </div>
    </div>
  );
}
