package dev.trywisp.mobile;

import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.security.GeneralSecurityException;
import java.security.KeyFactory;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.MessageDigest;
import java.security.PublicKey;
import java.security.spec.PKCS8EncodedKeySpec;
import java.security.spec.X509EncodedKeySpec;
import java.util.ArrayList;
import java.util.Base64;
import java.util.List;
import java.util.Properties;
import java.util.UUID;
import net.schmizz.sshj.DefaultConfig;
import net.schmizz.sshj.common.Buffer;
import net.schmizz.sshj.common.Factory;
import net.schmizz.sshj.common.KeyType;
import net.schmizz.sshj.userauth.keyprovider.FileKeyProvider;
import net.schmizz.sshj.userauth.keyprovider.KeyFormat;
import net.schmizz.sshj.userauth.keyprovider.KeyProviderUtil;
import net.schmizz.sshj.userauth.password.PasswordUtils;

/**
 * The phone's SSH keys, sealed in the {@link Vault} (the Android Keystore's key, ADR-0065 §2), never
 * in files or web storage. Each key is three entries: `key/<id>/priv` (PKCS#8), `key/<id>/pub`
 * (X.509) and `key/<id>/meta` (name, algorithm, made here or imported, when). Imported keys are
 * decrypted once on import, so their passphrase isn't kept. v1 kept them as files in the app's
 * private storage: {@link #moveV1} moves those in once and deletes them.
 * Plain Java, so the desktop check runs it too.
 */
public final class Keys {
    public static final class Info {
        public final String id, name, source;
        public final PublicKey pub;

        Info(String id, String name, String source, PublicKey pub) {
            this.id = id;
            this.name = name;
            this.source = source;
            this.pub = pub;
        }
    }

    private static final String PREFIX = "key/";
    private final Vault vault;

    public Keys(Vault vault) {
        this.vault = vault;
    }

    /**
     * v1's key files (`<id>.key`, `<id>.pub`, `<id>.properties` in `dir`) into the vault, then the
     * files and the folder are deleted. Their bytes move as they are, unread, so a key this phone
     * can't load stays listed as unreadable rather than lost. Answers how many keys moved; with no
     * folder left, nothing (it runs on every start and does its work once).
     */
    public static synchronized int moveV1(File dir, Vault vault) throws GeneralSecurityException, IOException {
        File[] files = dir.listFiles((d, n) -> n.endsWith(".properties"));
        if (files == null) return 0;
        java.util.Arrays.sort(files, (a, b) -> Long.compare(a.lastModified(), b.lastModified()));
        int n = 0;
        long at = 1;
        for (File f : files) {
            String id = f.getName().substring(0, f.getName().length() - ".properties".length());
            if (!id.matches("[a-z0-9]{1,32}")) continue;
            Properties p = new Properties();
            try (InputStream in = new FileInputStream(f)) {
                p.load(in);
            }
            if (vault.get(PREFIX + id + "/meta") == null) {
                p.setProperty("at", String.valueOf(at++));
                for (String ext : new String[] { "key", "pub" }) {
                    File part = new File(dir, id + "." + ext);
                    if (part.exists()) vault.put(PREFIX + id + "/" + (ext.equals("key") ? "priv" : "pub"), Files.readAllBytes(part.toPath()));
                }
                vault.put(PREFIX + id + "/meta", text(p));
                n++;
            }
        }
        // Everything is in the vault: now the files go (a crash before this moves them again, harmlessly).
        File[] all = dir.listFiles(File::isFile);
        if (all != null) for (File f : all) Files.deleteIfExists(f.toPath());
        try {
            Files.deleteIfExists(dir.toPath());
        } catch (java.nio.file.DirectoryNotEmptyException e) {
            // something that isn't v1's (a folder): left; the keys themselves are gone from here
        }
        return n;
    }

    public synchronized List<Info> list() throws IOException {
        return list(null);
    }

    /**
     * Every key that loads. One that doesn't (a damaged entry, an algorithm this phone's provider
     * lacks) is left in the vault and skipped, its name added to `unreadable`, so the others still list.
     */
    public synchronized List<Info> list(List<String> unreadable) throws IOException {
        List<Info> out = new ArrayList<>();
        List<String[]> metas = new ArrayList<>(); // [at, id]
        for (String name : vault.names(PREFIX)) {
            if (!name.endsWith("/meta")) continue;
            String id = name.substring(PREFIX.length(), name.length() - "/meta".length());
            Properties p = props(id);
            metas.add(new String[] { p == null ? "0" : p.getProperty("at", "0"), id });
        }
        metas.sort((a, b) -> {
            int c = Long.compare(parse(a[0]), parse(b[0]));
            return c != 0 ? c : a[1].compareTo(b[1]);
        });
        for (String[] m : metas) {
            String id = m[1];
            String name = id;
            try {
                Properties p = props(id);
                if (p == null) throw new IOException("no meta");
                name = p.getProperty("name", "Key");
                out.add(new Info(id, name, p.getProperty("source", "phone"), load(id).getPublic()));
            } catch (GeneralSecurityException | IOException | RuntimeException e) {
                if (unreadable != null) unreadable.add(name);
            }
        }
        return out;
    }

    private static long parse(String s) {
        try {
            return Long.parseLong(s);
        } catch (NumberFormatException e) {
            return 0;
        }
    }

    /** The phone's first key, made the first time the app needs one (or when none of them loads). */
    public synchronized List<Info> listOrCreate() throws GeneralSecurityException, IOException {
        return listOrCreate(null);
    }

    public synchronized List<Info> listOrCreate(List<String> unreadable) throws GeneralSecurityException, IOException {
        List<Info> l = list(unreadable);
        if (l.isEmpty()) {
            create("This phone");
            l = list();
        }
        return l;
    }

    public synchronized Info create(String name) throws GeneralSecurityException, IOException {
        KeyPair kp = KeyPairGenerator.getInstance("Ed25519", "BC").generateKeyPair();
        return save(name, "phone", kp);
    }

    /** An existing private key (OpenSSH, PEM or PuTTY), as pasted. */
    public synchronized Info importKey(String name, String privateKey, String passphrase) throws IOException, GeneralSecurityException {
        KeyFormat format = KeyProviderUtil.detectKeyFileFormat(privateKey, false);
        FileKeyProvider fkp = Factory.Named.Util.create(new DefaultConfig().getFileKeyProviderFactories(), format.toString());
        if (fkp == null) throw new IOException("That doesn’t look like a private key (OpenSSH, PEM or PuTTY).");
        fkp.init(privateKey, null, passphrase == null || passphrase.isEmpty() ? null : PasswordUtils.createOneOff(passphrase.toCharArray()));
        KeyPair kp;
        try {
            kp = new KeyPair(fkp.getPublic(), fkp.getPrivate());
        } catch (IOException e) {
            throw new IOException(passphrase == null || passphrase.isEmpty() ? "That key needs its passphrase." : "Wrong passphrase, or the key is damaged.");
        }
        return save(name, "imported", kp);
    }

    public synchronized void delete(String id) {
        for (String part : new String[] { "/priv", "/pub", "/meta" }) vault.remove(PREFIX + safe(id) + part);
    }

    public synchronized KeyPair load(String id) throws GeneralSecurityException, IOException {
        Properties p = props(safe(id));
        if (p == null) throw new IOException("No such key.");
        byte[] pub = vault.get(PREFIX + id + "/pub");
        byte[] priv = vault.get(PREFIX + id + "/priv");
        if (pub == null || priv == null) throw new IOException("That key is damaged.");
        KeyFactory kf = KeyFactory.getInstance(p.getProperty("alg", "Ed25519"), "BC");
        return new KeyPair(kf.generatePublic(new X509EncodedKeySpec(pub)), kf.generatePrivate(new PKCS8EncodedKeySpec(priv)));
    }

    private Info save(String name, String source, KeyPair kp) throws IOException, GeneralSecurityException {
        String id = UUID.randomUUID().toString().replace("-", "").substring(0, 12);
        Properties p = new Properties();
        p.setProperty("name", name == null || name.trim().isEmpty() ? "Key" : name.trim());
        p.setProperty("alg", kp.getPublic().getAlgorithm());
        p.setProperty("source", source);
        p.setProperty("at", String.valueOf(System.currentTimeMillis()));
        vault.put(PREFIX + id + "/priv", kp.getPrivate().getEncoded());
        vault.put(PREFIX + id + "/pub", kp.getPublic().getEncoded());
        vault.put(PREFIX + id + "/meta", text(p)); // last: a key is listed only once it is whole
        return new Info(id, p.getProperty("name"), source, kp.getPublic());
    }

    private static byte[] text(Properties p) throws IOException {
        java.io.ByteArrayOutputStream o = new java.io.ByteArrayOutputStream();
        p.store(o, null);
        return o.toByteArray();
    }

    private Properties props(String id) throws IOException {
        byte[] b = vault.get(PREFIX + id + "/meta");
        if (b == null) return null;
        Properties p = new Properties();
        p.load(new java.io.ByteArrayInputStream(b));
        return p;
    }

    // Ids come from the page: never let one name another entry.
    private static String safe(String id) {
        if (id == null || !id.matches("[a-z0-9]{1,32}")) throw new IllegalArgumentException("No such key.");
        return id;
    }

    /** The authorized_keys line: `ssh-ed25519 AAAA… wisp-phone`. */
    public static String authorizedKey(PublicKey key) {
        return KeyType.fromKey(key) + " " + Base64.getEncoder().encodeToString(blob(key)) + " wisp-phone";
    }

    /** `SHA256:…`, the way `ssh-keygen -lf` prints it, so it can be checked on the machine. */
    public static String fingerprint(PublicKey key) {
        try {
            byte[] sha = MessageDigest.getInstance("SHA-256").digest(blob(key));
            return "SHA256:" + Base64.getEncoder().withoutPadding().encodeToString(sha);
        } catch (GeneralSecurityException e) {
            throw new IllegalStateException(e);
        }
    }

    private static byte[] blob(PublicKey key) {
        return new Buffer.PlainBuffer().putPublicKey(key).getCompactData();
    }
}
