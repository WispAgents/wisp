package dev.trywisp.mobile;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.security.Security;
import java.util.List;
import org.bouncycastle.jce.provider.BouncyCastleProvider;
import org.junit.BeforeClass;
import org.junit.Test;

public class KeysTest {
    @BeforeClass
    public static void bc() {
        Security.removeProvider("BC");
        Security.insertProviderAt(new BouncyCastleProvider(), 1);
    }

    private static Keys fresh() throws Exception {
        return new Keys(TestVaults.vault());
    }

    @Test
    public void makesTheFirstKeyAndKeepsIt() throws Exception {
        Keys k = fresh();
        List<Keys.Info> first = k.listOrCreate();
        assertEquals(1, first.size());
        assertEquals("This phone", first.get(0).name);
        String line = Keys.authorizedKey(first.get(0).pub);
        assertTrue(line, line.matches("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5[A-Za-z0-9+/=]+ wisp-phone"));
        assertTrue(Keys.fingerprint(first.get(0).pub).matches("SHA256:[A-Za-z0-9+/]{43}"));
        // Same key next time, and it loads as a usable pair.
        assertEquals(Keys.fingerprint(first.get(0).pub), Keys.fingerprint(k.listOrCreate().get(0).pub));
        assertEquals(Keys.fingerprint(first.get(0).pub), Keys.fingerprint(k.load(first.get(0).id).getPublic()));
    }

    @Test
    public void createsDeletesAndRefusesOddIds() throws Exception {
        Keys k = fresh();
        Keys.Info a = k.create("Work");
        Keys.Info b = k.create("Home");
        assertEquals(2, k.list().size());
        k.delete(a.id);
        assertEquals(b.id, k.list().get(0).id);
        try {
            k.load("../../etc/passwd");
            fail("loaded a path");
        } catch (IllegalArgumentException expected) {
            // ids never leave the keys folder
        }
    }

    // One damaged key (a truncated entry, an unknown algorithm) is skipped and named; the rest list.
    @Test
    public void skipsAKeyThatDoesNotLoad() throws Exception {
        SealedVault vault = TestVaults.vault();
        Keys k = new Keys(vault);
        Keys.Info good = k.create("Good");
        Keys.Info cut = k.create("Cut");
        Keys.Info odd = k.create("Odd");
        vault.put("key/" + cut.id + "/pub", new byte[] { 1, 2, 3 });
        String meta = new String(vault.get("key/" + odd.id + "/meta"), StandardCharsets.UTF_8);
        vault.put("key/" + odd.id + "/meta", meta.replace("alg=Ed25519", "alg=Nope").getBytes(StandardCharsets.UTF_8));
        List<String> unreadable = new ArrayList<>();
        List<Keys.Info> l = k.list(unreadable);
        assertEquals(1, l.size());
        assertEquals(good.id, l.get(0).id);
        assertEquals(List.of("Cut", "Odd"), unreadable.stream().sorted().toList());
        // Nothing usable left: a fresh key is made; the damaged ones stay in the vault.
        k.delete(good.id);
        List<Keys.Info> fresh = k.listOrCreate();
        assertEquals(1, fresh.size());
        assertEquals("This phone", fresh.get(0).name);
        assertTrue(vault.get("key/" + cut.id + "/priv") != null);
    }

    // ssh-keygen output, plain and with a passphrase: the same fingerprint ssh-keygen -lf prints.
    @Test
    public void importsOpenSshKeys() throws Exception {
        File dir = Files.createTempDirectory("wisp-kg").toFile();
        for (String[] t : new String[][] { { "ed25519", "" }, { "rsa", "" }, { "ecdsa", "" }, { "ed25519", "s3cret" } }) {
            File f = new File(dir, "k-" + t[0] + t[1].length());
            Process p = new ProcessBuilder("ssh-keygen", "-q", "-t", t[0], "-N", t[1], "-f", f.getPath()).redirectErrorStream(true).start();
            assertEquals(0, p.waitFor());
            Process fp = new ProcessBuilder("ssh-keygen", "-lf", f.getPath() + ".pub").start();
            String expected = new String(fp.getInputStream().readAllBytes()).split(" ")[1];
            Keys k = fresh();
            String pem = new String(Files.readAllBytes(f.toPath()));
            if (!t[1].isEmpty()) {
                try {
                    k.importKey("x", pem, null);
                    fail("imported without the passphrase");
                } catch (Exception e) {
                    assertTrue(e.getMessage(), e.getMessage().contains("passphrase"));
                }
            }
            Keys.Info info = k.importKey("Laptop", pem, t[1]);
            assertEquals(t[0], expected, Keys.fingerprint(info.pub));
            assertEquals("imported", info.source);
            assertEquals(expected, Keys.fingerprint(k.load(info.id).getPublic()));
        }
    }

    @Test
    public void refusesThingsThatAreNotKeys() throws Exception {
        try {
            fresh().importKey("x", "hello", null);
            fail("imported garbage");
        } catch (Exception e) {
            assertTrue(e.getMessage(), e.getMessage().contains("private key"));
        }
    }

    // ---- ADR-0065 edge cases ----

    // v1's key files (files/keys/<id>.{key,pub,properties}) move into the Keystore-sealed vault on the
    // first v2 start, keep their ids, names and order, and the files are deleted. A second start
    // moves nothing (and makes no new key).
    @Test
    public void v1KeyFilesMoveIntoTheKeystoreOnce() throws Exception {
        File dir = Files.createTempDirectory("wisp-v1-keys").toFile();
        String[] ids = { "a1b2c3d4e5f6", "0f9e8d7c6b5a" };
        String[] names = { "This phone", "Laptop" };
        java.security.KeyPair[] pairs = new java.security.KeyPair[2];
        for (int i = 0; i < 2; i++) {
            pairs[i] = java.security.KeyPairGenerator.getInstance("Ed25519", "BC").generateKeyPair();
            Files.write(new File(dir, ids[i] + ".key").toPath(), pairs[i].getPrivate().getEncoded());
            Files.write(new File(dir, ids[i] + ".pub").toPath(), pairs[i].getPublic().getEncoded());
            Files.write(new File(dir, ids[i] + ".properties").toPath(), ("name=" + names[i] + "\nalg=Ed25519\nsource=" + (i == 0 ? "phone" : "imported") + "\n").getBytes(StandardCharsets.UTF_8));
            assertTrue(new File(dir, ids[i] + ".properties").setLastModified(1_700_000_000_000L + i * 1000));
        }
        TestVaults.MapShelf shelf = new TestVaults.MapShelf();
        SealedVault vault = TestVaults.vault(shelf);
        assertEquals(2, Keys.moveV1(dir, vault));
        assertFalse("the key files are gone", dir.exists());
        Keys k = new Keys(vault);
        List<Keys.Info> l = k.list();
        assertEquals(List.of(ids[0], ids[1]), l.stream().map(x -> x.id).toList());
        assertEquals(List.of(names[0], names[1]), l.stream().map(x -> x.name).toList());
        assertEquals("imported", l.get(1).source);
        for (int i = 0; i < 2; i++) {
            assertEquals(Keys.fingerprint(pairs[i].getPublic()), Keys.fingerprint(k.load(ids[i]).getPublic()));
            assertEquals(pairs[i].getPrivate(), k.load(ids[i]).getPrivate());
        }
        // Nothing on the shelf is the private key in the clear.
        String raw = String.join("\n", shelf.map.values());
        for (java.security.KeyPair p : pairs) assertFalse(raw.contains(java.util.Base64.getEncoder().encodeToString(p.getPrivate().getEncoded())));
        int puts = shelf.puts;
        assertEquals("only once", 0, Keys.moveV1(dir, vault));
        assertEquals(puts, shelf.puts);
        assertEquals(2, new Keys(vault).listOrCreate().size());
    }

    // A damaged v1 key still moves (its bytes, unread) and its file goes: the list names it unreadable.
    @Test
    public void aDamagedV1KeyMovesAndIsNamedUnreadable() throws Exception {
        File dir = Files.createTempDirectory("wisp-v1-keys").toFile();
        Files.write(new File(dir, "abc123.key").toPath(), new byte[] { 9, 9 });
        Files.write(new File(dir, "abc123.properties").toPath(), "name=Broken\nalg=Ed25519\n".getBytes(StandardCharsets.UTF_8));
        SealedVault vault = TestVaults.vault();
        assertEquals(1, Keys.moveV1(dir, vault));
        assertFalse(dir.exists());
        List<String> unreadable = new ArrayList<>();
        assertEquals(0, new Keys(vault).list(unreadable).size());
        assertEquals(List.of("Broken"), unreadable);
    }

    // A folder in v1's keys folder (not v1's own) doesn't stop the move or the keys loading after it.
    @Test
    public void aFolderInV1sKeysDoesNotBlockTheMove() throws Exception {
        File dir = Files.createTempDirectory("wisp-v1-keys").toFile();
        java.security.KeyPair kp = java.security.KeyPairGenerator.getInstance("Ed25519", "BC").generateKeyPair();
        Files.write(new File(dir, "abc123.key").toPath(), kp.getPrivate().getEncoded());
        Files.write(new File(dir, "abc123.pub").toPath(), kp.getPublic().getEncoded());
        Files.write(new File(dir, "abc123.properties").toPath(), "name=Phone\nalg=Ed25519\n".getBytes(StandardCharsets.UTF_8));
        assertTrue(new File(dir, "stray").mkdir());
        SealedVault vault = TestVaults.vault();
        assertEquals(1, Keys.moveV1(dir, vault));
        assertFalse(new File(dir, "abc123.key").exists());
        assertEquals(0, Keys.moveV1(dir, vault));
        assertEquals(1, new Keys(vault).list().size());
    }

    @Test
    public void aSealedVaultNeverShelvesPlaintext() throws Exception {
        TestVaults.MapShelf shelf = new TestVaults.MapShelf();
        SealedVault v = TestVaults.vault(shelf);
        byte[] secret = "wlt_device_token_123".getBytes(StandardCharsets.UTF_8);
        v.put("token/g1", secret);
        assertEquals("wlt_device_token_123", new String(v.get("token/g1"), StandardCharsets.UTF_8));
        String raw = String.join("\n", shelf.map.values()) + String.join("\n", shelf.map.keySet());
        assertFalse(raw, raw.contains("wlt_device_token_123"));
        assertFalse(raw.contains(java.util.Base64.getEncoder().encodeToString(secret)));
        assertEquals(List.of("token/g1"), v.names("token/"));
        v.remove("token/g1");
        assertNull(v.get("token/g1"));
        // A shelf entry sealed under another key (the Keystore key was wiped) reads as missing, not a crash.
        TestVaults.MapShelf other = new TestVaults.MapShelf();
        other.map.putAll(shelf.map);
        v.put("token/g2", secret);
        other.map.putAll(shelf.map);
        assertNull(TestVaults.vault(other).get("token/g2"));
    }

    // The password signs in once, adds the phone's key there (@@wisp keyed) and is dropped: the
    // reconnect after a lost connection signs in with the key and offers no password.
    @Test
    public void thePasswordIsForgottenAfterKeying() throws Exception {
        try (FakeSshd sshd = new FakeSshd("hunter2", FakeSshd.ec())) {
            List<String> lines = new java.util.concurrent.CopyOnWriteArrayList<>();
            java.security.KeyPair phone = FakeSshd.ec();
            Tunnel tn = Reach.tunnel("127.0.0.1", sshd.port(), "me", "hunter2", sshd.wispPort, "#!/bin/sh", phone, new HostTrust(new HostTrust.MapKnown(), null), lines::add);
            tn.start();
            TunnelTest.waitFor(lines, "@@wisp open", 1);
            assertTrue(String.join("\n", lines), lines.contains("@@wisp keyed"));
            assertEquals(1, sshd.passwordTries.get());
            assertFalse("the tunnel no longer holds the password", tn.hasPassword());
            tn.close(); // the connection drops: it comes back with the key
            TunnelTest.waitFor(lines, "@@wisp open", 2);
            assertEquals(String.join("\n", lines), "ready", tn.state());
            assertEquals("no password the second time", 1, sshd.passwordTries.get());
            tn.stop();
        }
    }
}
