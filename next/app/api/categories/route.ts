import { NextRequest, NextResponse } from "next/server";
import { getConfig } from "@/lib/config";
import { getOrFetch } from "@/lib/cache";

export async function GET(request: NextRequest) {
  try {
    const config = await getConfig();
    const action = request.nextUrl.searchParams.get("action");

    if (!action) {
      return NextResponse.json(
        { error: "Missing action parameter" },
        { status: 400 }
      );
    }

    const validActions = [
      "get_live_categories",
      "get_vod_categories",
      "get_series_categories",
    ];
    if (!validActions.includes(action)) {
      return NextResponse.json(
        { error: "Invalid action" },
        { status: 400 }
      );
    }

    const refresh = request.nextUrl.searchParams.get("refresh") === "true";

    const categories = await getOrFetch(
      config,
      `categories:${action}`,
      {},
      async () => {
        const url = `${config.address}/player_api.php?username=${config.username}&password=${config.password}&action=${action}`;

        const response = await fetch(url, {
          method: "GET",
        });

        if (!response.ok) {
          const error: any = new Error(`API returned status ${response.status}`);
          error.status = response.status;
          throw error;
        }

        const data = await response.json();

        return Array.isArray(data)
          ? data.map((item: any) => ({
              id: item.category_id || item.id || "",
              name: item.category_name || item.name || "",
            }))
          : [];
      },
      { forceRefresh: refresh }
    );

    return NextResponse.json(categories);
  } catch (error) {
    console.error("Error fetching categories:", error);
    const status = (error as any)?.status ?? 500;
    const message = (error as any)?.message ?? "Failed to fetch categories";
    return NextResponse.json(
      { error: message },
      { status }
    );
  }
}
