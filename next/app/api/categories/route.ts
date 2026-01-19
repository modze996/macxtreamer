import { NextRequest, NextResponse } from "next/server";
import { getConfig } from "@/lib/config";

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

    const url = `${config.address}/player_api.php?username=${config.username}&password=${config.password}&action=${action}`;

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

    // Validate and clean data
    const categories = Array.isArray(data)
      ? data.map((item: any) => ({
          id: item.category_id || item.id || "",
          name: item.category_name || item.name || "",
        }))
      : [];

    return NextResponse.json(categories);
  } catch (error) {
    console.error("Error fetching categories:", error);
    return NextResponse.json(
      { error: "Failed to fetch categories" },
      { status: 500 }
    );
  }
}
