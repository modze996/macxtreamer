import { NextRequest, NextResponse } from "next/server";
import { getConfig } from "@/lib/config";

export async function GET(request: NextRequest) {
  try {
    const config = await getConfig();
    const seriesId = request.nextUrl.searchParams.get("series_id");

    if (!seriesId) {
      return NextResponse.json(
        { error: "Missing series_id parameter" },
        { status: 400 }
      );
    }

    const url = `${config.address}/player_api.php?username=${config.username}&password=${config.password}&action=get_series_info&series_id=${seriesId}`;

    const response = await fetch(url, {
      method: "GET",
      timeout: 10000,
    });

    if (!response.ok) {
      return NextResponse.json(
        { error: `API returned status ${response.status}` },
        { status: response.status }
      );
    }

    const data = await response.json();

    const episodes: any[] = [];
    const seriesCover =
      data.info?.movie_image || data.info?.cover || null;

    if (data.episodes && typeof data.episodes === "object") {
      for (const season in data.episodes) {
        const seasonEpisodes = data.episodes[season];
        if (Array.isArray(seasonEpisodes)) {
          for (const ep of seasonEpisodes) {
            episodes.push({
              episodeId:
                ep.episode_id || ep.id || ep.stream_id || "",
              name: ep.title || ep.name || "",
              containerExtension:
                ep.container_extension || "mp4",
              streamUrl: ep.stream_url || null,
              cover: ep.cover || seriesCover,
            });
          }
        }
      }
    }

    return NextResponse.json(episodes);
  } catch (error) {
    console.error("Error fetching episodes:", error);
    return NextResponse.json(
      { error: "Failed to fetch episodes" },
      { status: 500 }
    );
  }
}
