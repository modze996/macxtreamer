import { NextRequest, NextResponse } from "next/server";
import { getConfig } from "@/lib/config";
import { getOrFetch } from "@/lib/cache";

export async function GET(request: NextRequest) {
  try {
    const config = await getConfig();
    const action = request.nextUrl.searchParams.get("action");
    // Accept both cat_id and category_id for flexibility
    const categoryId = request.nextUrl.searchParams.get("cat_id") || request.nextUrl.searchParams.get("category_id");

    if (!action || !categoryId) {
      return NextResponse.json(
        { error: "Missing action or cat_id parameter" },
        { status: 400 }
      );
    }

    const validActions = ["get_live_streams", "get_vod_streams", "get_series_streams"];
    if (!validActions.includes(action)) {
      return NextResponse.json(
        { error: "Invalid action" },
        { status: 400 }
      );
    }

    const refresh = request.nextUrl.searchParams.get("refresh") === "true";

    const items = await getOrFetch(
      config,
      `items:${action}`,
      { categoryId },
      async () => {
        const url = `${config.address}/player_api.php?username=${config.username}&password=${config.password}&action=${action}&category_id=${categoryId}`;

        const response = await fetch(url, {
          method: "GET",
        });

        if (!response.ok) {
          console.error(
            `[API Items] HTTP ${response.status} for action ${action}, cat_id ${categoryId}`
          );
          const error: any = new Error(`API returned status ${response.status}`);
          error.status = response.status;
          throw error;
        }

        let data;
        try {
          data = await response.json();
        } catch (err) {
          const text = await response.text();
          console.error(
            `[API Items] JSON parse error for action ${action}, cat_id ${categoryId}. Response: ${text.substring(0, 200)}`
          );
          const parseError: any = new Error("Invalid JSON response from API");
          parseError.status = 500;
          throw parseError;
        }

        return Array.isArray(data)
          ? data.map((item: any) => ({
              id: item.stream_id || item.series_id || item.id || "",
              name: item.name || item.title || "",
              image: item.cover || item.stream_icon || item.image || item.thumb || undefined,
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
      },
      { forceRefresh: refresh }
    );

    return NextResponse.json(items);
  } catch (error) {
    console.error("[API Items] Error:", error);
    return NextResponse.json(
      {
        error:
          error instanceof Error ? error.message : "Failed to fetch items",
      },
      { status: (error as any)?.status ?? 500 }
    );
  }
}
