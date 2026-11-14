package dev.trywisp.mobile;

import android.content.Context;
import java.nio.charset.StandardCharsets;
import java.security.GeneralSecurityException;
import java.util.Iterator;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * The computers this phone reaches (v2): `{id, kind: 'ssh'|'grid', name, peer?, push?, …}`. SSH ones
 * carry target, sshPort, port, keyId, command and the pinned host key `fingerprint` (from the QR
 * code); grid ones url, grid and computer. They live here, not in the page, because there's no
 * local wisp on a phone and Reach needs them to open a tunnel by id.
 *
 * A grid computer's device token never stays in this list (ADR-0065 §2): {@link #save} moves it
 * into the {@link Vault} (`token/<id>`), {@link #list} never answers it, and only connect's answer
 * (Reached) hands it to the page. A password is never kept. v1 kept the list in the computers
 * screen's localStorage; the bundle copies it in once (mobile/web migrate.ts) and deletes it there.
 * Backups are off for the app, as for the keys. Plain Java over a {@link SealedVault.Shelf}.
 */
final class Computers {
    static final String LIST = "list";
    static final String TOKEN = "token/";
    private static final String[] NEVER_KEPT = { "token", "password" };
    private static volatile Computers app;

    private final SealedVault.Shelf shelf;
    private final Vault vault;

    Computers(SealedVault.Shelf shelf, Vault vault) {
        this.shelf = shelf;
        this.vault = vault;
    }

    static Computers of(Context c) throws GeneralSecurityException, java.io.IOException {
        Computers cs = app;
        if (cs != null) return cs;
        synchronized (Computers.class) {
            if (app == null) app = new Computers(KeystoreVault.shelf(c.getApplicationContext().getSharedPreferences("wisp.computers", 0)), KeystoreVault.of(c));
            return app;
        }
    }

    synchronized JSONArray list() {
        JSONArray all;
        try {
            all = new JSONArray(shelf.get(LIST) == null ? "[]" : shelf.get(LIST));
        } catch (JSONException e) {
            return new JSONArray();
        }
        // An earlier v2 build kept device tokens here: move them into the vault, once.
        boolean scrubbed = false;
        boolean failed = false;
        for (int i = 0; i < all.length(); i++) {
            JSONObject o = all.optJSONObject(i);
            if (o != null && (o.has("token") || o.has("password"))) {
                try {
                    keepSecrets(o);
                    scrubbed = true;
                } catch (GeneralSecurityException | RuntimeException e) {
                    // the vault refused: the shelf keeps the entry to try again next read, the answer never has it
                    for (String k : NEVER_KEPT) o.remove(k);
                    failed = true;
                }
            }
        }
        if (scrubbed && !failed) shelf.put(LIST, all.toString());
        return all;
    }

    synchronized JSONObject get(String id) {
        JSONArray all = list();
        for (int i = 0; i < all.length(); i++) {
            JSONObject o = all.optJSONObject(i);
            if (o != null && id.equals(o.optString("id"))) return o;
        }
        return null;
    }

    /** The device token for a grid computer, for connect's answer only; "" when there is none. */
    synchronized String token(String id) {
        byte[] t = vault.get(TOKEN + id);
        return t == null ? "" : new String(t, StandardCharsets.UTF_8);
    }

    /** connect's answer for a grid computer (Reached): the one place its device token leaves the vault. */
    synchronized JSONObject gridReached(String id) throws JSONException {
        JSONObject c = get(id);
        if (c == null || !"grid".equals(c.optString("kind"))) return null;
        JSONObject r = new JSONObject().put("kind", "grid").put("base", c.optString("url")).put("token", token(id)).put("name", c.optString("name", id));
        if (!c.optString("peer").isEmpty()) r.put("peer", c.optString("peer"));
        return r;
    }

    /** Adds it, or merges its fields into the saved one (a learnt peer id, a new token, push paired). */
    synchronized JSONObject save(JSONObject computer) throws JSONException, GeneralSecurityException {
        String id = computer.optString("id", "");
        if (id.isEmpty()) throw new JSONException("a computer needs an id");
        computer = new JSONObject(computer.toString()); // the caller's object keeps its fields
        JSONArray all = list();
        JSONObject existing = null;
        for (int i = 0; i < all.length(); i++) {
            JSONObject o = all.optJSONObject(i);
            if (o != null && id.equals(o.optString("id"))) existing = o;
        }
        if (existing == null && !computer.has("kind")) throw new JSONException("a new computer needs a kind");
        // A QR pin is set when the computer is added; later only Trust the new key changes it (pin()).
        if (existing != null) computer.remove("fingerprint");
        keepSecrets(computer);
        JSONArray out = new JSONArray();
        JSONObject merged = null;
        for (int i = 0; i < all.length(); i++) {
            JSONObject o = all.optJSONObject(i);
            if (o == null) continue;
            if (o == existing) {
                for (Iterator<String> k = computer.keys(); k.hasNext();) {
                    String key = k.next();
                    o.put(key, computer.get(key));
                }
                merged = o;
            }
            out.put(o);
        }
        if (merged == null) {
            merged = computer;
            out.put(computer);
        }
        shelf.put(LIST, out.toString());
        return merged;
    }

    /** Trust the new key replaced a QR pin (WispSshPlugin.trustHostKey checked it was the one shown). */
    synchronized void pin(String id, String fingerprint) throws JSONException {
        JSONArray all = list();
        for (int i = 0; i < all.length(); i++) {
            JSONObject o = all.optJSONObject(i);
            if (o != null && id.equals(o.optString("id"))) o.put("fingerprint", fingerprint);
        }
        shelf.put(LIST, all.toString());
    }

    synchronized void forget(String id) {
        JSONArray all = list();
        JSONArray out = new JSONArray();
        for (int i = 0; i < all.length(); i++) {
            JSONObject o = all.optJSONObject(i);
            if (o != null && !id.equals(o.optString("id"))) out.put(o);
        }
        shelf.put(LIST, out.toString());
        vault.remove(TOKEN + id);
    }

    // The token into the vault, the password nowhere; both leave the object.
    private void keepSecrets(JSONObject o) throws GeneralSecurityException {
        String token = o.optString("token", "");
        if (!token.isEmpty()) vault.put(TOKEN + o.optString("id"), token.getBytes(StandardCharsets.UTF_8));
        for (String k : NEVER_KEPT) o.remove(k);
    }
}
