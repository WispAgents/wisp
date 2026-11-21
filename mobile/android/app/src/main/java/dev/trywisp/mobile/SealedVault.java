package dev.trywisp.mobile;

import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.security.GeneralSecurityException;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Base64;
import java.util.Collections;
import java.util.List;
import java.util.Set;
import javax.crypto.Cipher;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;

/**
 * Secrets (the phone's SSH keys, device tokens) sealed with a key that never leaves the Android
 * Keystore (ADR-0065 §2): AES-256-GCM, so the shelf (SharedPreferences) only ever holds ciphertext.
 * Each value is sealed with its name as associated data, so a value moved under another name fails
 * to open. Names on the shelf are `v:` + the name; names aren't secret, values are.
 * Plain Java: the JVM tests run it with a plain AES key in place of the Keystore's.
 */
public final class SealedVault implements Vault {
    public interface Sealer {
        byte[] seal(byte[] plain, byte[] aad) throws GeneralSecurityException;

        byte[] open(byte[] sealed, byte[] aad) throws GeneralSecurityException;
    }

    /** Where the ciphertext is kept: string keys to string values. */
    public interface Shelf {
        String get(String key);

        void put(String key, String value);

        void remove(String key);

        Set<String> keys();
    }

    private static final String PREFIX = "v:";
    private final Sealer sealer;
    private final Shelf shelf;

    public SealedVault(Sealer sealer, Shelf shelf) {
        this.sealer = sealer;
        this.shelf = shelf;
    }

    /** AES-GCM with a 12-byte IV the cipher picks (the Keystore insists on that), stored before the ciphertext. */
    public static Sealer aesGcm(SecretKey key) {
        return new Sealer() {
            @Override
            public byte[] seal(byte[] plain, byte[] aad) throws GeneralSecurityException {
                Cipher c = Cipher.getInstance("AES/GCM/NoPadding");
                c.init(Cipher.ENCRYPT_MODE, key);
                c.updateAAD(aad);
                byte[] iv = c.getIV();
                byte[] body = c.doFinal(plain);
                return ByteBuffer.allocate(1 + iv.length + body.length).put((byte) iv.length).put(iv).put(body).array();
            }

            @Override
            public byte[] open(byte[] sealed, byte[] aad) throws GeneralSecurityException {
                if (sealed.length < 2) throw new GeneralSecurityException("short");
                int n = sealed[0] & 0xff;
                if (sealed.length < 1 + n) throw new GeneralSecurityException("short");
                Cipher c = Cipher.getInstance("AES/GCM/NoPadding");
                c.init(Cipher.DECRYPT_MODE, key, new GCMParameterSpec(128, sealed, 1, n));
                c.updateAAD(aad);
                return c.doFinal(sealed, 1 + n, sealed.length - 1 - n);
            }
        };
    }

    @Override
    public synchronized byte[] get(String name) {
        String s = shelf.get(PREFIX + name);
        if (s == null) return null;
        try {
            return sealer.open(Base64.getDecoder().decode(s), aad(name));
        } catch (GeneralSecurityException | RuntimeException e) {
            // sealed under a key that is gone (app data restored elsewhere), or the Keystore failed
            // (ProviderException): as if missing; Keys lists it as unreadable, the entry stays
            return null;
        }
    }

    @Override
    public synchronized void put(String name, byte[] value) throws GeneralSecurityException {
        shelf.put(PREFIX + name, Base64.getEncoder().encodeToString(sealer.seal(value, aad(name))));
    }

    @Override
    public synchronized void remove(String name) {
        shelf.remove(PREFIX + name);
    }

    @Override
    public synchronized List<String> names(String prefix) {
        List<String> out = new ArrayList<>();
        for (String k : shelf.keys()) if (k.startsWith(PREFIX + prefix)) out.add(k.substring(PREFIX.length()));
        Collections.sort(out);
        return out;
    }

    private static byte[] aad(String name) {
        try {
            return MessageDigest.getInstance("SHA-256").digest(("wisp.vault:" + name).getBytes(StandardCharsets.UTF_8));
        } catch (GeneralSecurityException e) {
            throw new IllegalStateException(e);
        }
    }
}
