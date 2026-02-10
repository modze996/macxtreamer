"use client";

import Image from "next/image";
import { useRef } from "react";

interface EpgProgram {
  id: string;
  title: string;
  startTime: string;
  endTime: string;
  duration: number; // in minutes
}

interface TvChannel {
  id: string;
  name: string;
  logoUrl?: string;
  category: string;
  programs: EpgProgram[];
}

interface TvGuideProps {
  channels: TvChannel[];
  loading?: boolean;
}

const categoryColors: Record<string, string> = {
  bundesliga: "bg-blue-600",
  "2bundesliga": "bg-purple-600",
  dazn: "bg-red-900",
  default: "bg-gray-700",
};

export default function TvGuide({ channels, loading = false }: TvGuideProps) {
  const timelineRef = useRef<HTMLDivElement>(null);

  const getCategoryColor = (category: string) => {
    return categoryColors[category.toLowerCase()] || categoryColors.default;
  };

  return (
    <div className="mb-8">
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-2xl font-semibold text-white">TV-Guide</h2>
        {loading && (
          <div className="flex items-center gap-2 text-gray-400 text-sm">
            <div className="animate-spin h-4 w-4 border-2 border-yellow-500 border-t-transparent rounded-full" />
            <span>EPG wird geladen...</span>
          </div>
        )}
      </div>

      <div className="bg-[#0a0a0a] rounded-xl border border-gray-800 overflow-hidden">
        <div className="space-y-3 p-4">
          {channels.map((channel) => (
            <div
              key={channel.id}
              className="flex items-center gap-4 hover:bg-gray-800/30 rounded-lg p-3 transition-colors"
            >
              {/* Channel Logo and Name */}
              <div className="flex items-center gap-3 w-48 flex-shrink-0">
                {channel.logoUrl ? (
                  <div className="relative w-10 h-10 rounded-md overflow-hidden bg-gray-800">
                    <Image
                      src={channel.logoUrl}
                      alt={channel.name}
                      fill
                      className="object-contain"
                    />
                  </div>
                ) : (
                  <div className="w-10 h-10 rounded-md bg-gray-800 flex items-center justify-center text-gray-500 text-xs font-bold">
                    {channel.name.substring(0, 2)}
                  </div>
                )}
                <div className="flex-1 min-w-0">
                  <h3 className="text-sm font-medium text-white truncate">
                    {channel.name}
                  </h3>
                  <p className="text-xs text-gray-500 truncate">
                    {channel.category}
                  </p>
                </div>
              </div>

              {/* Timeline with Programs */}
              <div
                ref={timelineRef}
                className="flex-1 flex gap-1 overflow-x-auto scrollbar-hide"
                style={{ scrollbarWidth: "none", msOverflowStyle: "none" }}
              >
                {channel.programs.map((program) => {
                  // Calculate width based on duration (1 minute = 2px as base)
                  const width = Math.max(program.duration * 2, 80);
                  
                  return (
                    <div
                      key={program.id}
                      className={`flex-shrink-0 ${getCategoryColor(
                        channel.category
                      )} rounded-md px-3 py-2 transition-all hover:brightness-110 cursor-pointer`}
                      style={{ minWidth: `${width}px` }}
                    >
                      <div className="flex flex-col h-full justify-center">
                        <p className="text-xs font-medium text-white truncate">
                          {program.title}
                        </p>
                        <p className="text-xs text-white/70 mt-0.5">
                          {program.startTime} - {program.endTime}
                        </p>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
