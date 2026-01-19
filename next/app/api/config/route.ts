import { NextRequest, NextResponse } from "next/server";
import { loadConfig } from "@/lib/config";

export async function GET(request: NextRequest) {
  try {
    const config = await loadConfig();

    if (!config) {
      return NextResponse.json(
        { configured: false, error: "No configuration found" },
        { status: 200 }
      );
    }

    // Validate the config by making a test API call
    try {
      const testUrl = `${config.address}/player_api.php?username=${config.username}&password=${config.password}&action=get_live_categories`;
      const response = await fetch(testUrl, {
        method: "GET",
        timeout: 5000,
      });

      return NextResponse.json({
        configured: true,
        valid: response.ok,
        address: config.address,
        username: config.username,
        // Don't return the password
      });
    } catch (error) {
      return NextResponse.json({
        configured: true,
        valid: false,
        address: config.address,
        username: config.username,
        error: "Failed to validate configuration",
      });
    }
  } catch (error) {
    console.error("Error checking config:", error);
    return NextResponse.json(
      { error: "Failed to check configuration" },
      { status: 500 }
    );
  }
}
