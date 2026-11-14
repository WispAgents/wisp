package dev.trywisp.mobile;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNull;

import java.nio.charset.StandardCharsets;
import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Test;

public class ComputersTest {
    private static String token(SealedVault v, String id) {
        byte[] t = v.get(Computers.TOKEN + id);
        return t == null ? null : new String(t, StandardCharsets.UTF_8);
    }

    // ADR-0065 §2: a grid computer's device token goes into the Keystore-sealed vault; the saved
    // list (what the page reads with list()) never carries it, nor does the shelf on disk.
    @Test
    public void aGridTokenIsKeptInTheVaultNotTheList() throws Exception {
        TestVaults.MapShelf shelf = new TestVaults.MapShelf();
        SealedVault vault = TestVaults.vault(shelf);
        Computers cs = new Computers(shelf, vault);
        cs.save(new JSONObject().put("id", "g1").put("kind", "grid").put("name", "wisp Cloud").put("url", "https://mo.grid.test").put("token", "wlt_device_1"));
        JSONObject listed = cs.list().getJSONObject(0);
        assertFalse(listed.has("token"));
        assertEquals("https://mo.grid.test", listed.getString("url"));
        assertEquals("wlt_device_1", cs.token("g1"));
        String raw = String.join("\n", shelf.map.values());
        assertFalse(raw, raw.contains("wlt_device_1"));
        // a renewed token (a patch) replaces it; forgetting the computer drops it
        cs.save(new JSONObject().put("id", "g1").put("token", "wlt_device_2").put("peer", "box-1"));
        assertEquals("wlt_device_2", cs.token("g1"));
        assertEquals("box-1", cs.get("g1").getString("peer"));
        assertFalse(cs.get("g1").has("token"));
        cs.forget("g1");
        assertNull(token(vault, "g1"));
        assertEquals(0, cs.list().length());
    }

    @Test
    public void aPasswordIsNeverSaved() throws Exception {
        TestVaults.MapShelf shelf = new TestVaults.MapShelf();
        Computers cs = new Computers(shelf, TestVaults.vault(shelf));
        cs.save(new JSONObject().put("id", "s1").put("kind", "ssh").put("target", "me@studio").put("password", "hunter2"));
        assertFalse(cs.get("s1").has("password"));
        assertFalse(String.join("\n", shelf.map.values()).contains("hunter2"));
    }

    // The QR pin can't be changed by a patch from the page; only Trust the new key (pin()) does.
    @Test
    public void aPinChangesOnlyThroughTrust() throws Exception {
        TestVaults.MapShelf shelf = new TestVaults.MapShelf();
        Computers cs = new Computers(shelf, TestVaults.vault(shelf));
        cs.save(new JSONObject().put("id", "s1").put("kind", "ssh").put("target", "me@studio").put("fingerprint", "SHA256:qr"));
        cs.save(new JSONObject().put("id", "s1").put("fingerprint", "SHA256:sneaky").put("name", "Studio"));
        assertEquals("SHA256:qr", cs.get("s1").getString("fingerprint"));
        assertEquals("Studio", cs.get("s1").getString("name"));
        cs.pin("s1", "SHA256:trusted");
        assertEquals("SHA256:trusted", cs.get("s1").getString("fingerprint"));
    }

    // The vault refuses (the Keystore failed): the token still never comes out of list().
    @Test
    public void aTokenTheVaultCantTakeIsStillNotListed() throws Exception {
        TestVaults.MapShelf shelf = new TestVaults.MapShelf();
        Vault broken = new Vault() {
            public byte[] get(String n) { return null; }
            public void put(String n, byte[] v) { throw new java.security.ProviderException("keystore"); }
            public void remove(String n) {}
            public java.util.List<String> names(String p) { return java.util.List.of(); }
        };
        shelf.put(Computers.LIST, new JSONArray().put(new JSONObject().put("id", "g1").put("kind", "grid").put("token", "wlt_old")).toString());
        Computers cs = new Computers(shelf, broken);
        assertFalse(cs.list().toString().contains("wlt_old"));
        assertEquals("kept on the shelf to move next time", true, shelf.get(Computers.LIST).contains("wlt_old"));
    }

    // An earlier v2 build kept the token in the list: the first read moves it into the vault, once.
    @Test
    public void anEarlierBuildsTokenLeavesTheListOnce() throws Exception {
        TestVaults.MapShelf shelf = new TestVaults.MapShelf();
        SealedVault vault = TestVaults.vault(shelf);
        shelf.put(Computers.LIST, new JSONArray().put(new JSONObject().put("id", "g1").put("kind", "grid").put("url", "https://x").put("token", "wlt_old")).toString());
        Computers cs = new Computers(shelf, vault);
        assertFalse(cs.list().getJSONObject(0).has("token"));
        assertEquals("wlt_old", cs.token("g1"));
        assertFalse(shelf.get(Computers.LIST).contains("wlt_old"));
    }

    // The recording mobile/web's bridge-contract suite replays (fixtures/android-bridge.json): every
    // answer in it is what Computers gives here, so the TypeScript fake can't drift from the Java.
    @Test
    public void theRecordedBridgeIsComputersOwn() throws Exception {
        java.io.File f = new java.io.File("../../web/test/fixtures/android-bridge.json");
        JSONObject rec = new JSONObject(new String(java.nio.file.Files.readAllBytes(f.toPath()), StandardCharsets.UTF_8));
        TestVaults.MapShelf shelf = new TestVaults.MapShelf();
        Computers cs = new Computers(shelf, TestVaults.vault(shelf));
        JSONArray steps = rec.getJSONArray("steps");
        for (int i = 0; i < steps.length(); i++) {
            JSONObject s = steps.getJSONObject(i);
            JSONObject args = s.getJSONObject("args");
            Object got;
            switch (s.getString("method")) {
                case "saveComputer": got = cs.save(args.getJSONObject("computer")); break;
                case "list": got = new JSONObject().put("computers", cs.list()); break;
                case "connect": got = cs.gridReached(args.getString("id")); break;
                case "forgetComputer": cs.forget(args.getString("id")); got = new JSONObject(); break;
                default: throw new AssertionError("not recorded here: " + s.getString("method"));
            }
            org.junit.Assert.assertTrue("step " + i + " " + s.getString("method") + ": " + got + " vs " + s.get("answer"), same(got, s.get("answer")));
        }
    }

    // JSON equality, key order aside (android.jar's org.json, which tests compile against, has no similar()).
    private static boolean same(Object a, Object b) throws Exception {
        if (a instanceof JSONObject && b instanceof JSONObject) {
            JSONObject x = (JSONObject) a, y = (JSONObject) b;
            if (x.length() != y.length()) return false;
            for (java.util.Iterator<String> k = x.keys(); k.hasNext();) {
                String key = k.next();
                if (!y.has(key) || !same(x.get(key), y.get(key))) return false;
            }
            return true;
        }
        if (a instanceof JSONArray && b instanceof JSONArray) {
            JSONArray x = (JSONArray) a, y = (JSONArray) b;
            if (x.length() != y.length()) return false;
            for (int i = 0; i < x.length(); i++) if (!same(x.get(i), y.get(i))) return false;
            return true;
        }
        return a != null && a.equals(b);
    }
}
