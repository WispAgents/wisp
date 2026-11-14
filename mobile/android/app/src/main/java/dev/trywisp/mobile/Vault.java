package dev.trywisp.mobile;

import java.security.GeneralSecurityException;
import java.util.List;

/** Named secrets kept sealed on the phone (see {@link SealedVault}, {@link KeystoreVault}). */
public interface Vault {
    /** The value, or null when there is none (or it no longer opens). */
    byte[] get(String name);

    void put(String name, byte[] value) throws GeneralSecurityException;

    void remove(String name);

    /** Every name starting with `prefix`, sorted. */
    List<String> names(String prefix);
}
