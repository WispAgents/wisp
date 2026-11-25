package dev.trywisp.mobile;

import android.content.SharedPreferences;
import android.webkit.WebView;
import com.getcapacitor.JSArray;
import com.getcapacitor.JSObject;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;
import java.security.KeyPair;
import java.security.Security;
import org.bouncycastle.jce.provider.BouncyCastleProvider;
import org.json.JSONObject;

/**
 * The page's side of {@link Tunnel} and {@link Keys}: listKeys, createKey, importKey, deleteKey,
 * connect, status, disconnect, forgetHost, and a `line` event per `@@wisp …` line.
 *
 * One machine is connected at a time (the phone's 127.0.0.1:port can only lead to one wisp), so
 * connecting another stops the first; wisp and its agents keep running on both. The tunnel is
 * static so it outlives the page: the WebView leaves this page for wisp's own and comes back.
 * While one is connected, {@link TunnelService} keeps the app running in the background.
 *
 * v2 (the bundle in the APK, mobile/web): every saved computer at once. list, saveComputer,
 * forgetComputer ({@link Computers}); connect({id}) answers how to reach it ({@link Reach}), and
 * disconnect({id}) lets one go; leave sends the app to the background. Events: `tunnel` {id, state,
 * line}, `computers` (the list changed), `back` (Android's Back, when the bundle listens) and `open`
 * {target} (a tapped notification). Called with `host`, connect is still v1's single tunnel.
 *
 * ADR-0065 §2: keys and device tokens are sealed with the Android Keystore's key ({@link
 * KeystoreVault}); v1's key files move in on the first start ({@link Keys#moveV1}). A computer
 * scanned from the desktop's QR code carries its host key `fingerprint`, pinned before the first
 * connect ({@link HostTrust}); trustHostKey({id, fingerprint}) is *Trust the new key*, after the
 * page's confirmation. The foreground service runs only as {@link Stay} says (setStayConnected).
 */
@CapacitorPlugin(name = "WispSsh")
public class WispSshPlugin extends Plugin {
    private static Tunnel tunnel;
    private static String machineId = "";
    private static volatile String cloudOrigin; // the cloud computer the list opened (stayOpen)
    private static volatile WispSshPlugin self; // the page's plugin: v2's tunnels outlive a reload
    private static final Reach reach = new Reach((id, state, line) -> {
        WispSshPlugin p = self;
        if (p == null) return;
        JSObject ev = new JSObject();
        ev.put("id", id);
        ev.put("state", state);
        ev.put("line", line);
        p.notifyListeners("tunnel", ev);
        if ("failed".equals(state)) p.restay(); // a tunnel that gave up holds nothing
    });
    private static volatile boolean serving; // TunnelService started for v2's computers
    private String template; // assets/wisp/bootstrap.sh
    private Keys keys;
    private static final String STAY = "on";

    @Override
    public void load() {
        // Android ships a cut-down "BC" provider without Ed25519: put the full one first.
        Security.removeProvider("BC");
        Security.insertProviderAt(new BouncyCastleProvider(), 1);
        self = this;
        // The first v2 start moves v1's key files and any kept token into the vault, before anything asks.
        Thread t = new Thread(() -> {
            try {
                keys();
                computers().list();
            } catch (Exception ignored) {
                // tried again by the first call that needs them
            }
        }, "wisp-vault");
        t.setDaemon(true);
        t.start();
    }

    // Back from the background: check the tunnel now rather than when keep-alives give up.
    @Override
    protected void handleOnResume() {
        Tunnel t = tunnel;
        if (t != null) t.nudge();
        reach.nudgeAll();
        restay(); // a foreground start refused while the app was behind goes through now
    }

    private synchronized Keys keys() throws java.security.GeneralSecurityException, java.io.IOException {
        if (keys == null) {
            SealedVault vault = KeystoreVault.of(getContext());
            Keys.moveV1(new java.io.File(getContext().getFilesDir(), "keys"), vault);
            keys = new Keys(vault);
        }
        return keys;
    }

    private Computers computers() throws java.security.GeneralSecurityException, java.io.IOException {
        return Computers.of(getContext());
    }

    private static JSObject json(Keys.Info k) {
        JSObject o = new JSObject();
        o.put("id", k.id);
        o.put("name", k.name);
        o.put("source", k.source);
        o.put("type", net.schmizz.sshj.common.KeyType.fromKey(k.pub).toString());
        o.put("publicKey", Keys.authorizedKey(k.pub));
        o.put("fingerprint", Keys.fingerprint(k.pub));
        return o;
    }

    private SharedPreferences hosts() {
        return getContext().getSharedPreferences("wisp.hosts", 0);
    }

    private HostTrust.Known known() {
        SharedPreferences p = hosts();
        return new HostTrust.Known() {
            @Override
            public String get(String hostPort) {
                return p.getString(hostPort, null);
            }

            @Override
            public void put(String hostPort, String fingerprint) {
                p.edit().putString(hostPort, fingerprint).apply();
            }
        };
    }

    // A wisp Grid signed in to: its pages (the grid and <handle>.<grid>) load in the app.
    @PluginMethod
    public void allowGrid(PluginCall call) {
        Grid.allow(getContext(), call.getString("domain", ""));
        call.resolve();
    }

    // Into a cloud computer (no SSH): the app must still stay up in the background, or Android
    // freezes it once it's off screen and wisp's page never hears that a task finished or asks.
    // Called before leaving the list, while the app is on screen (a foreground service can't start
    // from the background). The list stops it again when it's back with no tunnel (disconnect).
    @PluginMethod
    public void stayOpen(PluginCall call) {
        cloudOrigin = Shared.originOf(call.getString("origin"));
        TunnelService.start(getContext(), call.getString("name", ""));
        call.resolve();
    }

    // The grid's sign-in page goes to the browser: Google and others refuse embedded WebViews.
    @PluginMethod
    public void openExternal(PluginCall call) {
        String url = call.getString("url", "");
        if (!url.startsWith("https://") && !url.startsWith("http://")) {
            call.reject("Not a web address.");
            return;
        }
        android.content.Intent i = new android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(url));
        i.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK);
        getContext().startActivity(i);
        call.resolve();
    }

    // Share: Android's own chooser. The System WebView has no navigator.share and the app carries no
    // Share plugin, so the page's Share was a silent copy. The chooser tells nothing of what was picked:
    // once it is open, it's shared.
    @PluginMethod
    public void share(PluginCall call) {
        String title = call.getString("title", "");
        String text = call.getString("text", "");
        String url = call.getString("url", "");
        String body = text.isEmpty() ? url : url.isEmpty() || text.contains(url) ? text : text + "\n" + url;
        if (body.isEmpty() && title.isEmpty()) {
            call.reject("Nothing to share.");
            return;
        }
        android.content.Intent send = new android.content.Intent(android.content.Intent.ACTION_SEND);
        send.setType("text/plain");
        send.putExtra(android.content.Intent.EXTRA_TEXT, body.isEmpty() ? title : body);
        if (!title.isEmpty()) send.putExtra(android.content.Intent.EXTRA_SUBJECT, title);
        android.content.Intent chooser = android.content.Intent.createChooser(send, title.isEmpty() ? null : title);
        getActivity().runOnUiThread(() -> {
            try {
                getActivity().startActivity(chooser);
                JSObject r = new JSObject();
                r.put("shared", true);
                call.resolve(r);
            } catch (android.content.ActivityNotFoundException e) {
                call.reject("No app takes a share.", e);
            }
        });
    }

    // Scan the desktop's *Add to your phone* QR: Google's code scanner (Play services draws the camera
    // screen, so the app asks for no CAMERA permission). {text} scanned, {cancelled} closed, {denied}
    // when the scanner module is missing or can't run: the page then asks for the address by hand.
    @PluginMethod
    public void scanCode(PluginCall call) {
        com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions options =
            new com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions.Builder()
                .setBarcodeFormats(com.google.mlkit.vision.barcode.common.Barcode.FORMAT_QR_CODE)
                .build();
        JSObject cancelled = new JSObject();
        cancelled.put("cancelled", true);
        try {
            com.google.mlkit.vision.codescanner.GmsBarcodeScanning.getClient(getActivity(), options)
                .startScan()
                .addOnSuccessListener(code -> {
                    String text = code.getRawValue();
                    if (text == null || text.isEmpty()) {
                        call.resolve(cancelled);
                        return;
                    }
                    JSObject r = new JSObject();
                    r.put("text", text);
                    call.resolve(r);
                })
                .addOnCanceledListener(() -> call.resolve(cancelled))
                .addOnFailureListener(e -> call.resolve(denied()));
        } catch (RuntimeException e) {
            call.resolve(denied()); // no Play services on this phone
        }
    }

    private static JSObject denied() {
        JSObject r = new JSObject();
        r.put("denied", true);
        return r;
    }

    @PluginMethod
    public void listKeys(PluginCall call) {
        try {
            JSArray list = new JSArray();
            java.util.List<String> unreadable = new java.util.ArrayList<>();
            for (Keys.Info k : keys().listOrCreate(unreadable)) list.put(json(k));
            JSObject r = new JSObject();
            r.put("keys", list);
            r.put("unreadable", new JSArray(unreadable));
            call.resolve(r);
        } catch (Exception e) {
            call.reject("Couldn’t read this phone’s keys: " + e.getMessage(), e);
        }
    }

    @PluginMethod
    public void createKey(PluginCall call) {
        try {
            call.resolve(json(keys().create(call.getString("name", "Key"))));
        } catch (Exception e) {
            call.reject("Couldn’t make a key: " + e.getMessage(), e);
        }
    }

    @PluginMethod
    public void importKey(PluginCall call) {
        try {
            call.resolve(json(keys().importKey(call.getString("name", "Imported key"), call.getString("privateKey", ""), call.getString("passphrase"))));
        } catch (Exception e) {
            call.reject(e.getMessage() != null ? e.getMessage() : "Couldn’t read that key.", e);
        }
    }

    @PluginMethod
    public void deleteKey(PluginCall call) {
        try {
            keys().delete(call.getString("id"));
            call.resolve();
        } catch (Exception e) {
            call.reject(e.getMessage(), e);
        }
    }

    @PluginMethod
    public void connect(PluginCall call) {
        if (!call.getData().has("host") && call.getData().has("id")) {
            reachConnect(call);
            return;
        }
        String host = call.getString("host", "").trim();
        String user = call.getString("user", "").trim();
        String script = call.getString("script", "");
        int sshPort = call.getInt("sshPort", 22);
        int port = call.getInt("port", 4477);
        if (host.isEmpty() || user.isEmpty() || script.isEmpty()) {
            call.reject("Need user@host and the bootstrap script.");
            return;
        }
        KeyPair kp;
        try {
            kp = keys().load(call.getString("keyId"));
        } catch (Exception e) {
            call.reject("This machine’s key is gone: pick another one.", e);
            return;
        }
        synchronized (WispSshPlugin.class) {
            if (tunnel != null) tunnel.stop();
            machineId = call.getString("machineId", "");
            tunnel = new Tunnel(host, sshPort, user, call.getString("password"), port, script, kp, new HostTrust(known(), null), (text) -> {
                JSObject ev = new JSObject();
                ev.put("text", text);
                notifyListeners("line", ev);
                toPage(text);
            });
            tunnel.start();
        }
        TunnelService.start(getContext(), call.getString("name", host));
        call.resolve();
    }

    // wisp's own page (http://127.0.0.1:<port>) has no Capacitor bridge, but the app may show it
    // before the tunnel is up (from its cache): it hears the same lines as a `wisp-tunnel` event
    // (web/js/features/live.js), and reads where things are with window.wispAndroid.tunnel().
    private void toPage(String text) {
        if (getBridge() == null) return;
        WebView web = getBridge().getWebView();
        web.post(() -> {
            String url = web.getUrl();
            if (!Shared.isPage(url, pageOrigin())) return;
            web.evaluateJavascript("window.dispatchEvent(new CustomEvent('wisp-tunnel', { detail: " + JSONObject.quote(text) + " }))", null);
        });
    }

    /** Where wisp's page is on the computer open now, or null: the tunnel's port, or the cloud computer. */
    static String pageOrigin() {
        Tunnel t = tunnel;
        return t != null ? "http://127.0.0.1:" + t.port : cloudOrigin;
    }

    static boolean hasTunnel() {
        return tunnel != null;
    }

    /** wisp's page failed to load: check the tunnel now (a reconnect starts wisp again). */
    static void nudgeTunnel() {
        Tunnel t = tunnel;
        if (t != null) t.nudge();
    }

    /** {state, line}: the tunnel's state and its last progress line, for wisp's page as it opens. */
    static String pageState() {
        JSObject r = new JSObject();
        Tunnel t = tunnel;
        java.util.List<String> lines = t == null ? java.util.Collections.emptyList() : t.lines();
        r.put("state", t == null ? "idle" : t.state());
        r.put("line", lines.isEmpty() ? "" : lines.get(lines.size() - 1));
        return r.toString();
    }

    @PluginMethod
    public void forgetHost(PluginCall call) {
        hosts().edit().remove(call.getString("host", "") + ":" + call.getInt("sshPort", 22)).apply();
        call.resolve();
    }

    @PluginMethod
    public void status(PluginCall call) {
        JSObject r = new JSObject();
        Tunnel t = tunnel;
        r.put("state", t == null ? "idle" : t.state());
        r.put("machineId", machineId);
        r.put("port", t == null ? 4477 : t.port);
        r.put("lines", new JSArray(t == null ? java.util.Collections.emptyList() : t.lines()));
        call.resolve(r);
    }

    @PluginMethod
    public void disconnect(PluginCall call) {
        String id = call.getString("id");
        if (id != null) {
            reach.disconnect(id);
            restay();
            call.resolve();
            return;
        }
        stopTunnel();
        cloudOrigin = null;
        TunnelService.stop(getContext());
        call.resolve();
    }

    static void stopTunnel() {
        synchronized (WispSshPlugin.class) {
            if (tunnel != null) tunnel.stop();
            tunnel = null;
            machineId = "";
        }
        reach.stopAll();
        serving = false;
        stayFor = "";
    }

    // ---- v2: the bundle reaches every computer at once ----

    // Never with a token or password in it (Computers keeps them out of the list).
    @PluginMethod
    public void list(PluginCall call) {
        try {
            call.resolve(new JSObject().put("computers", computers().list()));
        } catch (Exception e) {
            call.reject("Couldn’t read the saved computers: " + e.getMessage(), e);
        }
    }

    // The page's SecureStore (wisp Cloud's device token): sealed in the vault, never in web storage.
    // A reject here makes the page answer 'unsupported' and refuse the sign-in, so only reject on a real failure.
    private Secrets secrets() throws java.security.GeneralSecurityException, java.io.IOException {
        return new Secrets(KeystoreVault.of(getContext()));
    }

    @PluginMethod
    public void secureGet(PluginCall call) {
        try {
            String v = secrets().get(call.getString("key", ""));
            JSObject o = new JSObject();
            o.put("value", v == null ? JSONObject.NULL : v);
            call.resolve(o);
        } catch (Exception e) {
            call.reject("Couldn’t open the sealed store: " + e.getMessage(), e);
        }
    }

    @PluginMethod
    public void secureSet(PluginCall call) {
        try {
            secrets().set(call.getString("key", ""), call.getString("value", ""));
            call.resolve(new JSObject().put("set", true));
        } catch (Exception e) {
            call.reject("Couldn’t seal it: " + e.getMessage(), e);
        }
    }

    @PluginMethod
    public void secureRemove(PluginCall call) {
        try {
            secrets().remove(call.getString("key", ""));
            call.resolve();
        } catch (Exception e) {
            call.reject("Couldn’t open the sealed store: " + e.getMessage(), e);
        }
    }

    // A grid computer's token goes into the vault here; the answer (the saved entry) has none.
    @PluginMethod
    public void saveComputer(PluginCall call) {
        try {
            JSObject saved = JSObject.fromJSONObject(computers().save(call.getObject("computer", new JSObject())));
            notifyListeners("computers", new JSObject());
            restay(); // push paired or unpaired
            call.resolve(saved);
        } catch (Exception e) {
            call.reject(e.getMessage(), e);
        }
    }

    @PluginMethod
    public void forgetComputer(PluginCall call) {
        String id = call.getString("id", "");
        reach.disconnect(id);
        try {
            computers().forget(id);
        } catch (Exception e) {
            call.reject(e.getMessage(), e);
            return;
        }
        restay();
        notifyListeners("computers", new JSObject());
        call.resolve();
    }

    // *Trust the new key*, after the page's confirmation: only the fingerprint the refusal showed.
    @PluginMethod
    public void trustHostKey(PluginCall call) {
        String id = call.getString("id", "");
        String fp = call.getString("fingerprint", "");
        try {
            org.json.JSONObject c = computers().get(id);
            if (c == null || !"ssh".equals(c.optString("kind"))) {
                call.reject("No saved computer " + id + ".");
                return;
            }
            Bootstrap.Target t = Bootstrap.target(c.optString("target", ""), c.optInt("sshPort", 22));
            if (!HostTrust.trustNew(known(), HostTrust.hostPort(t.host, t.sshPort), fp, reach.refusedKey(id))) {
                call.reject("That isn’t the key this computer showed. Connect again to see it.");
                return;
            }
            // A pin from the QR code is replaced by the key the user just trusted.
            if (!c.optString("fingerprint").isEmpty()) computers().pin(id, fp);
            call.resolve();
        } catch (Exception e) {
            call.reject(e.getMessage(), e);
        }
    }

    // Settings → Phone → *Stay connected* (off until the user turns it on).
    @PluginMethod
    public void setStayConnected(PluginCall call) {
        getContext().getSharedPreferences("wisp.stay", 0).edit().putBoolean(STAY, Boolean.TRUE.equals(call.getBoolean("on", false))).apply();
        restay();
        call.resolve(new JSObject().put("on", stayConnected()));
    }

    @PluginMethod
    public void getStayConnected(PluginCall call) {
        call.resolve(new JSObject().put("on", stayConnected()));
    }

    private boolean stayConnected() {
        return getContext().getSharedPreferences("wisp.stay", 0).getBoolean(STAY, false);
    }

    // Back with nothing left to close in the bundle: the app goes behind, its tunnels kept.
    @PluginMethod
    public void leave(PluginCall call) {
        getActivity().runOnUiThread(() -> getActivity().moveTaskToBack(true));
        call.resolve();
    }

    private void reachConnect(PluginCall call) {
        String id = call.getString("id", "");
        org.json.JSONObject c;
        Computers cs;
        try {
            cs = computers();
            c = cs.get(id);
        } catch (Exception e) {
            call.reject("Couldn’t read the saved computers: " + e.getMessage(), e);
            return;
        }
        if (c == null) {
            call.reject("No saved computer " + id + "."); // forgotten meanwhile
            return;
        }
        String name = c.optString("name", id);
        if ("grid".equals(c.optString("kind"))) {
            try {
                call.resolve(JSObject.fromJSONObject(cs.gridReached(id))); // the device token leaves the vault only here
            } catch (org.json.JSONException e) {
                call.reject(e.getMessage(), e);
            }
            return;
        }
        try {
            reach.connect(id, name, sshTunnel(id, name, c, call.getString("password")), call);
            restay();
        } catch (Exception e) {
            call.reject(e.getMessage() != null ? e.getMessage() : e.toString(), e);
        }
    }

    private Tunnel sshTunnel(String id, String name, org.json.JSONObject c, String password) throws Exception {
        Bootstrap.Target t = Bootstrap.target(c.optString("target", ""), c.optInt("sshPort", 22));
        if (t.user.isEmpty() || t.host.isEmpty()) throw new Exception("Add your user name: you@its-name");
        int port = c.optInt("port", 4477);
        String keyId = c.optString("keyId", "");
        if (keyId.isEmpty()) keyId = keys().listOrCreate().get(0).id; // as v1's keyOf
        KeyPair kp = keys().load(keyId);
        String script = Bootstrap.fill(template(), port, c.optString("command", ""));
        Tunnel[] box = new Tunnel[1];
        HostTrust trust = new HostTrust(known(), c.optString("fingerprint", ""));
        box[0] = Reach.tunnel(t.host, t.sshPort, t.user, password, port, script, kp, trust, (text) -> reach.line(id, name, box[0], text));
        return box[0];
    }

    private synchronized String template() throws java.io.IOException {
        if (template == null) {
            try (java.io.InputStream in = getContext().getAssets().open(Bootstrap.ASSET)) {
                template = new String(in.readAllBytes(), java.nio.charset.StandardCharsets.UTF_8);
            }
        }
        return template;
    }

    // The foreground service, as Stay says: an SSH computer without push connected, and the user's
    // Stay connected switch on (v1 ran it whenever a computer was connected). Its notification names
    // the computer when there is one, else says how many.
    private static volatile String stayFor = "";

    private void restay() {
        java.util.List<org.json.JSONObject> held = new java.util.ArrayList<>();
        try {
            Computers cs = computers();
            for (String id : reach.ids()) held.add(cs.get(id));
        } catch (Exception ignored) {
            // nothing readable: nothing to hold
        }
        boolean need = Stay.needed(stayConnected(), held);
        String name = "";
        if (need) {
            int n = 0;
            for (org.json.JSONObject c : held) if (c != null && "ssh".equals(c.optString("kind")) && !c.optBoolean("push", false)) {
                n++;
                name = c.optString("name", "");
            }
            if (n > 1) name = n + " computers";
        }
        synchronized (WispSshPlugin.class) {
            // TunnelService.isRunning: a start refused in the background is tried again here (on resume)
            if (need && (!serving || !TunnelService.isRunning() || !name.equals(stayFor))) {
                serving = true;
                stayFor = name;
                TunnelService.start(getContext(), name);
            } else if (!need && serving) {
                serving = false;
                stayFor = "";
                if (tunnel == null && cloudOrigin == null) TunnelService.stop(getContext()); // v1's page holds its own
            }
        }
    }

    /** Android's Back: true when the bundle listens for it (it closes what's open, or calls leave). */
    static boolean back() {
        WispSshPlugin p = self;
        if (p == null || !p.hasListeners("back")) return false;
        p.notifyListeners("back", new JSObject());
        return true;
    }

    /** A tapped notification (Notifications.schedule): kept until the bundle listens, on a cold start. */
    static void opened(String target) {
        WispSshPlugin p = self;
        if (p == null) return;
        JSObject ev = new JSObject();
        ev.put("target", target);
        p.notifyListeners("open", ev, true);
    }
}
