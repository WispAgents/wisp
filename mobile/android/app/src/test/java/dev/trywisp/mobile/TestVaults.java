package dev.trywisp.mobile;

import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import javax.crypto.KeyGenerator;

/** The vault as the app has it, with a plain AES key standing in for the Android Keystore's. */
final class TestVaults {
    private TestVaults() {}

    /** A shelf that is a map the test can read: what SharedPreferences would hold on disk. */
    static final class MapShelf implements SealedVault.Shelf {
        final Map<String, String> map = new TreeMap<>();
        int puts;

        @Override
        public synchronized String get(String k) {
            return map.get(k);
        }

        @Override
        public synchronized void put(String k, String v) {
            puts++;
            map.put(k, v);
        }

        @Override
        public synchronized void remove(String k) {
            map.remove(k);
        }

        @Override
        public synchronized Set<String> keys() {
            return new java.util.TreeSet<>(map.keySet());
        }
    }

    static SealedVault vault(MapShelf shelf) throws Exception {
        KeyGenerator g = KeyGenerator.getInstance("AES");
        g.init(256);
        return new SealedVault(SealedVault.aesGcm(g.generateKey()), shelf);
    }

    static SealedVault vault() throws Exception {
        return vault(new MapShelf());
    }
}
