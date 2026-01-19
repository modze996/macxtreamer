import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "MacXStreamer Web",
  description: "Web frontend for MacXStreamer IPTV player",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="de">
      <body className="bg-gray-900 text-white">
        {children}
      </body>
    </html>
  );
}
