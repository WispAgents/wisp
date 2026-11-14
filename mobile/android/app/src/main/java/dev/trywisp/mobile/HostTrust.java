package dev.trywisp.mobile;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Whose host key a connect accepts (ADR-0065 §2). A fingerprint pinned from the desktop's QR code
 * (`wisp+ssh://…?fp=SHA256:…`) is the only key accepted, from the first connect on. Typed by hand,
 * the first key is shown and kept (trust on first use, as v1). A key other than the kept one is
 * refused. Either refusal happens in the key exchange, before any password or key is sent; the
 * page offers *Trust the new key* behind a confirmation, which calls {@link #trustNew}.
 */
final class HostTrust implements Tunnel.HostKeys {
    /** The fingerprints kept per `host:port` (SharedPreferences `wisp.hosts`; not secret). */
    interface Known {
        String get(String hostPort);

        void put(String hostPort, String fingerprint);
    }

    static final class MapKnown implements Known {
        private final Map<String, String> map = new ConcurrentHashMap<>();

        @Override
        public String get(String hostPort) {
            return map.get(hostPort);
        }

        @Override
        public void put(String hostPort, String fingerprint) {
            map.put(hostPort, fingerprint);
        }
    }

    static final String CHANGED = "This computer’s key changed: it is now ";
    static final String NOT_PINNED = "This computer’s key isn’t the one its QR code showed: it is now ";

    private final Known known;
    private final String pinned;

    HostTrust(Known known, String pinned) {
        this.known = known;
        this.pinned = pinned == null || pinned.isEmpty() ? null : pinned;
    }

    static String hostPort(String host, int port) {
        return host + ":" + port;
    }

    @Override
    public String refuse(String host, int port, String fingerprint) {
        String id = hostPort(host, port);
        if (pinned != null) {
            if (!pinned.equals(fingerprint)) return NOT_PINNED + fingerprint + ".";
            if (!fingerprint.equals(known.get(id))) known.put(id, fingerprint);
            return null;
        }
        String kept = known.get(id);
        if (kept == null) {
            known.put(id, fingerprint);
            return null;
        }
        return kept.equals(fingerprint) ? null : CHANGED + fingerprint + ".";
    }

    /**
     * Trust the new key: keeps `fingerprint` only when it is the one the refusal showed (`shown`),
     * so a confirmation for an older refusal can't trust whatever the computer shows next.
     */
    static boolean trustNew(Known known, String hostPort, String fingerprint, String shown) {
        if (fingerprint == null || fingerprint.isEmpty() || !fingerprint.equals(shown)) return false;
        known.put(hostPort, fingerprint);
        return true;
    }

    /** The new fingerprint in a refusal's message, or null when it isn't one. */
    static String changedTo(String message) {
        for (String p : new String[] { CHANGED, NOT_PINNED }) {
            if (message != null && message.startsWith(p) && message.endsWith(".")) return message.substring(p.length(), message.length() - 1);
        }
        return null;
    }
}
