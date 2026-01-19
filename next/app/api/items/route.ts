import { NextRequest, NextResponse } from "next/server";
import { getConfig } from "@/lib/config";

export async function GET(request: NextRequest) {
  try {
    const config = await getConfig();
    const action = request.nextUrl.searchParams.get("action");
    const categoryId = request.nextUrl.searchParams.get("category_id");

    if (!action || !categoryId) {
      return NextResponse.json(
        { error: "Missing action or category_id parameter" },
        { status: 400 }
      );
    }

    const validActions = ["get_live_streams", "get_vod_streams", "get_series"];
    if (!validActions.includes(action)) {
      return NextResponse.json(
        { error: "Invalid action" },
        { status: 400 }
      );
    }

    const url = `${config.address}/player_api.php?username=${config.username}&password=${config.password}&action=${action}&category_id=${categoryId}`;

    const response = await fetch(url, {
      method: "GET",
    });

    if (!response.ok) {
      return NextResponse.json(
        { error: `API returned status ${response.status}` },
        { status: response.status }
      );
    }

    const data = await response.json();

    // Validate and clean data
    const items = Array.isArray(data)
      ? data.map((item: any) => ({
          id: item.stream_id || item.series_id || item.id || "",
          name: item.name || "",
          cover: item.cover || item.stream_icon || null,
          plot: item.plot || "",
          containerExtension: item.container_extension || "mp4",
          streamUrl: item.stream_url || null,
          year: item.year || null,
          rating: item.rating_5based || item.rating || null,
          genre: item.genre || null,
          director: item.director || null,
          cast: item.cast || null,
          audioLanguages: item.audio_languages || null,
        }))
      : [];

    return NextResponse.json(items);
  } catch (error) {
    console.error("Error fetching items:", error);
    return NextResponse.json(
      { error: "Failed to fetch items" },
      { status: 500 }
    );
  }
}
