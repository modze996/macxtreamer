"use client";

import { Search } from "lucide-react";
import { useState } from "react";

interface TopBarProps {
  title: string;
  onSearch?: (query: string) => void;
}

export default function TopBar({ title, onSearch }: TopBarProps) {
  const [searchQuery, setSearchQuery] = useState("");

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault();
    if (onSearch) {
      onSearch(searchQuery);
    }
  };

  return (
    <div className="flex flex-col md:flex-row md:items-center justify-between mb-8 gap-4">
      <h1 className="text-2xl md:text-3xl font-bold text-white">{title}</h1>
      
      <form onSubmit={handleSearch} className="relative w-full md:w-80">
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder="Suchen..."
          className="w-full bg-gray-800/50 border border-gray-700 rounded-lg px-4 py-2.5 pl-11 text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-yellow-500/50 focus:border-yellow-500/50 transition-all"
        />
        <Search 
          className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" 
          size={20} 
        />
      </form>
    </div>
  );
}
