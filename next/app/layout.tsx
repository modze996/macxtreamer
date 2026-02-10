import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "nextreamer - IPTV Streaming",
  description: "Modern IPTV streaming platform with Next.js",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="de">
      <body className="bg-[#050505] text-white antialiased">
        {children}
      </body>
    </html>
  );
}
