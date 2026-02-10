import React from "react";

export function ContentSkeleton({ type = "poster" }: { type?: "poster" | "landscape" }) {
  return (
    <div className="animate-pulse">
      <div
        className={`bg-gray-800 rounded-lg ${
          type === "poster" ? "w-44 h-64" : "w-80 h-44"
        }`}
      />
      <div className="mt-3">
        <div className="h-4 bg-gray-800 rounded w-3/4 mb-2" />
        <div className="h-3 bg-gray-800 rounded w-1/2" />
      </div>
    </div>
  );
}

export function HorizontalRowSkeleton({ type = "poster" }: { type?: "poster" | "landscape" }) {
  const items = Array.from({ length: 6 });
  
  return (
    <div className="mb-10">
      <div className="flex items-center justify-between mb-4">
        <div className="h-8 bg-gray-800 rounded w-48 animate-pulse" />
        <div className="flex gap-2">
          <div className="h-10 w-10 bg-gray-800 rounded-lg animate-pulse" />
          <div className="h-10 w-10 bg-gray-800 rounded-lg animate-pulse" />
        </div>
      </div>
      
      <div className="flex gap-4 overflow-hidden">
        {items.map((_, index) => (
          <ContentSkeleton key={index} type={type} />
        ))}
      </div>
    </div>
  );
}

export function ContinueWatchingSkeleton() {
  const items = Array.from({ length: 4 });
  
  return (
    <div className="mb-10">
      <div className="flex items-center justify-between mb-4">
        <div className="h-8 bg-gray-800 rounded w-48 animate-pulse" />
        <div className="flex gap-2">
          <div className="h-10 w-10 bg-gray-800 rounded-lg animate-pulse" />
          <div className="h-10 w-10 bg-gray-800 rounded-lg animate-pulse" />
        </div>
      </div>
      
      <div className="flex gap-4 overflow-hidden">
        {items.map((_, index) => (
          <div key={index} className="flex-shrink-0 w-96 animate-pulse">
            <div className="bg-gray-800 rounded-xl h-56" />
            <div className="mt-3">
              <div className="h-4 bg-gray-800 rounded w-3/4 mb-2" />
              <div className="h-3 bg-gray-800 rounded w-1/2" />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export function TvGuideSkeleton() {
  const channels = Array.from({ length: 5 });
  
  return (
    <div className="mb-8">
      <div className="h-8 bg-gray-800 rounded w-32 mb-6 animate-pulse" />
      
      <div className="bg-[#0a0a0a] rounded-xl border border-gray-800 overflow-hidden">
        <div className="space-y-3 p-4">
          {channels.map((_, index) => (
            <div key={index} className="flex items-center gap-4 animate-pulse">
              <div className="flex items-center gap-3 w-48 flex-shrink-0">
                <div className="w-10 h-10 rounded-md bg-gray-800" />
                <div className="flex-1">
                  <div className="h-4 bg-gray-800 rounded w-full mb-2" />
                  <div className="h-3 bg-gray-800 rounded w-2/3" />
                </div>
              </div>
              
              <div className="flex-1 flex gap-1">
                <div className="flex-shrink-0 bg-gray-800 rounded-md h-16 w-32" />
                <div className="flex-shrink-0 bg-gray-800 rounded-md h-16 w-24" />
                <div className="flex-shrink-0 bg-gray-800 rounded-md h-16 w-40" />
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

export function CategoryGridSkeleton() {
  const categories = Array.from({ length: 10 });
  
  return (
    <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-4">
      {categories.map((_, index) => (
        <div
          key={index}
          className="bg-gray-900/50 rounded-lg p-6 border border-gray-800 animate-pulse"
        >
          <div className="flex flex-col items-center justify-center text-center h-full">
            <div className="w-16 h-16 mb-4 rounded-full bg-gray-800" />
            <div className="h-4 bg-gray-800 rounded w-3/4" />
          </div>
        </div>
      ))}
    </div>
  );
}
