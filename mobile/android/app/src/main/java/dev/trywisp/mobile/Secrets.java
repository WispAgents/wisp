package dev.trywisp.mobile;

import java.nio.charset.StandardCharsets;
import java.security.GeneralSecurityException;

/**
 * The page's sealed store (`@wisp/shell/platform` SecureStore: secureGet/secureSet/secureRemove):
 * wisp Cloud's device tokens and whatever else the page must never keep in web storage. Each value
 * is sealed in the app's {@link SealedVault} under `secure/<key>`, apart from {@link Keys}' `key/` and
 * {@link Computers}' `token/`, so the page can't read or overwrite those.
 */
final class Secrets {
    static final String PREFIX = "secure/";
    private final Vault vault;

    Secrets(Vault vault) {
        this.vault = vault;
    }

    String get(String key) {
        if (key.isEmpty()) return null;
        byte[] v = vault.get(PREFIX + key);
        return v == null ? null : new String(v, StandardCharsets.UTF_8);
    }

    void set(String key, String value) throws GeneralSecurityException {
        if (key.isEmpty()) throw new IllegalArgumentException("no key");
        vault.put(PREFIX + key, value.getBytes(StandardCharsets.UTF_8));
    }

    void remove(String key) {
        if (!key.isEmpty()) vault.remove(PREFIX + key);
    }
}
