package dev.trywisp.mobile;

import android.content.Context;
import android.content.SharedPreferences;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import java.security.GeneralSecurityException;
import java.security.KeyStore;
import java.util.Set;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;

/**
 * The app's {@link SealedVault}: an AES-256-GCM key made in the Android Keystore (hardware-backed
 * where the phone has it; it can't be read out, only used), and the ciphertext in the private
 * `wisp.vault` preferences (backups are off for the app). Ed25519 keys can't live in the Keystore
 * itself before Android 13 and imported ones never could, so each key is sealed with this one.
 */
final class KeystoreVault {
    private static final String ALIAS = "wisp.vault";
    private static volatile SealedVault vault;

    private KeystoreVault() {}

    static SealedVault of(Context ctx) throws GeneralSecurityException, java.io.IOException {
        SealedVault v = vault;
        if (v != null) return v;
        synchronized (KeystoreVault.class) {
            if (vault == null) vault = new SealedVault(SealedVault.aesGcm(key()), shelf(ctx.getApplicationContext().getSharedPreferences("wisp.vault", 0)));
            return vault;
        }
    }

    private static SecretKey key() throws GeneralSecurityException, java.io.IOException {
        KeyStore ks = KeyStore.getInstance("AndroidKeyStore");
        ks.load(null);
        KeyStore.Entry e = ks.getEntry(ALIAS, null);
        if (e instanceof KeyStore.SecretKeyEntry) return ((KeyStore.SecretKeyEntry) e).getSecretKey();
        KeyGenerator g = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore");
        g.init(new KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .build());
        return g.generateKey();
    }

    static SealedVault.Shelf shelf(SharedPreferences p) {
        return new SealedVault.Shelf() {
            @Override
            public String get(String key) {
                return p.getString(key, null);
            }

            // commit, not apply: a secret written is on disk before the call that wrote it answers
            @Override
            public void put(String key, String value) {
                p.edit().putString(key, value).commit();
            }

            @Override
            public void remove(String key) {
                p.edit().remove(key).commit();
            }

            @Override
            public Set<String> keys() {
                return new java.util.TreeSet<>(p.getAll().keySet());
            }
        };
    }
}
