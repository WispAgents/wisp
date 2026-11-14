package dev.trywisp.mobile;

import org.json.JSONObject;

/**
 * When the foreground service (TunnelService) runs (ADR-0065 §3.6): only while an SSH computer
 * without a push channel is connected, and only with the user's *Stay connected* switch. A grid
 * computer has no tunnel to hold; one with push gets its alerts without the app running.
 */
final class Stay {
    private Stay() {}

    static boolean needed(boolean stayConnected, Iterable<JSONObject> connected) {
        if (!stayConnected) return false;
        for (JSONObject c : connected) {
            if (c != null && "ssh".equals(c.optString("kind")) && !c.optBoolean("push", false)) return true;
        }
        return false;
    }
}
