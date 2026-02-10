"use client";

import { useState } from "react";
import Link from "next/link";
import { Home, Heart, Tv, Film, Calendar, Download, Radio, Settings, Plus, X, Menu } from "lucide-react";

interface XtreamAccount {
  id: string;
  name: string;
}

interface SidebarProps {
  accounts?: XtreamAccount[];
}

export default function Sidebar({ accounts = [] }: SidebarProps) {
  const [activeItem, setActiveItem] = useState("start");
  const [expandedAccount, setExpandedAccount] = useState<string | null>(null);
  const [mobileOpen, setMobileOpen] = useState(false);

  const menuItems = [
    { id: "start", label: "Start", icon: Home, href: "/" },
    { id: "favoriten", label: "Favoriten", icon: Heart, href: "/favoriten" },
  ];

  const bottomItems = [
    { id: "downloads", label: "Downloads", icon: Download, href: "/downloads" },
    { id: "aufnahmen", label: "Aufnahmen", icon: Radio, href: "/aufnahmen" },
    { id: "einstellungen", label: "Einstellungen", icon: Settings, href: "/einstellungen" },
    { id: "add-playlist", label: "Playlist hinzufügen", icon: Plus, href: "/add-playlist" },
  ];

  const accountSubItems = [
    { id: "live-tv", label: "Live TV", icon: Tv },
    { id: "serien", label: "Serien", icon: Film },
    { id: "filme", label: "Filme", icon: Film },
    { id: "tv-guide", label: "TV-Guide", icon: Calendar },
  ];

  const closeMobile = () => setMobileOpen(false);

  return (
    <>
      {/* Mobile Menu Button */}
      <button
        onClick={() => setMobileOpen(!mobileOpen)}
        className="lg:hidden fixed top-4 left-4 z-50 p-2 rounded-lg bg-gray-800 text-white hover:bg-gray-700 transition-colors"
        aria-label="Toggle menu"
      >
        {mobileOpen ? <X size={24} /> : <Menu size={24} />}
      </button>

      {/* Overlay for mobile */}
      {mobileOpen && (
        <div
          className="lg:hidden fixed inset-0 bg-black/50 z-40"
          onClick={closeMobile}
        />
      )}

      {/* Sidebar */}
      <div
        className={`w-64 bg-[#0a0a0a] h-screen fixed left-0 top-0 flex flex-col border-r border-gray-800 z-40 transition-transform duration-300 ${
          mobileOpen ? "translate-x-0" : "-translate-x-full lg:translate-x-0"
        }`}
      >
      {/* Logo */}
      <div className="p-6 border-b border-gray-800">
        <h1 className="text-2xl font-bold text-white">nextreamer</h1>
      </div>

      {/* Main Navigation */}
      <nav className="flex-1 overflow-y-auto py-4">
        <div className="space-y-1 px-3">
          {menuItems.map((item) => {
            const Icon = item.icon;
            const isActive = activeItem === item.id;
            
            return (
              <Link
                key={item.id}
                href={item.href}
                onClick={() => {
                  setActiveItem(item.id);
                  closeMobile();
                }}
                className={`flex items-center gap-3 px-4 py-3 rounded-lg transition-all relative ${
                  isActive
                    ? "bg-gray-800/50 text-white"
                    : "text-gray-400 hover:text-white hover:bg-gray-800/30"
                }`}
              >
                {isActive && (
                  <div className="absolute left-0 w-1 h-8 bg-yellow-500 rounded-r-full" />
                )}
                <Icon size={20} />
                <span className="text-sm font-medium">{item.label}</span>
              </Link>
            );
          })}

          {/* Xtream Accounts */}
          {accounts.length > 0 && (
            <div className="mt-6">
              <div className="px-4 py-2 text-xs font-semibold text-gray-500 uppercase tracking-wider">
                Accounts
              </div>
              {accounts.map((account) => (
                <div key={account.id} className="mt-1">
                  <button
                    onClick={() =>
                      setExpandedAccount(
                        expandedAccount === account.id ? null : account.id
                      )
                    }
                    className="w-full flex items-center gap-3 px-4 py-3 rounded-lg text-gray-400 hover:text-white hover:bg-gray-800/30 transition-all"
                  >
                    <Tv size={20} />
                    <span className="text-sm font-medium flex-1 text-left">
                      {account.name}
                    </span>
                    <svg
                      className={`w-4 h-4 transition-transform ${
                        expandedAccount === account.id ? "rotate-90" : ""
                      }`}
                      fill="none"
                      viewBox="0 0 24 24"
                      stroke="currentColor"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M9 5l7 7-7 7"
                      />
                    </svg>
                  </button>

                  {expandedAccount === account.id && (
                    <div className="ml-8 mt-1 space-y-1">
                      {accountSubItems.map((subItem) => {
                        const SubIcon = subItem.icon;
                        return (
                          <button
                            key={subItem.id}
                            className="w-full flex items-center gap-3 px-4 py-2 rounded-lg text-gray-400 hover:text-white hover:bg-gray-800/30 transition-all text-sm"
                          >
                            <SubIcon size={16} />
                            <span>{subItem.label}</span>
                          </button>
                        );
                      })}
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Bottom Items */}
        <div className="mt-auto pt-6 border-t border-gray-800 px-3 space-y-1">
          {bottomItems.map((item) => {
            const Icon = item.icon;
            const isActive = activeItem === item.id;
            
            return (
              <Link
                key={item.id}
                href={item.href}
                onClick={() => {
                  setActiveItem(item.id);
                  closeMobile();
                }}
                className={`flex items-center gap-3 px-4 py-3 rounded-lg transition-all relative ${
                  isActive
                    ? "bg-gray-800/50 text-white"
                    : "text-gray-400 hover:text-white hover:bg-gray-800/30"
                }`}
              >
                {isActive && (
                  <div className="absolute left-0 w-1 h-8 bg-yellow-500 rounded-r-full" />
                )}
                <Icon size={20} />
                <span className="text-sm font-medium">{item.label}</span>
              </Link>
            );
          })}
        </div>
      </nav>
      </div>
    </>
  );
}
