package dev.trywisp.mobile;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;
import static org.junit.Assume.assumeTrue;

import java.nio.file.Files;
import java.nio.file.Paths;
import java.security.Security;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import org.bouncycastle.jce.provider.BouncyCastleProvider;
import org.junit.BeforeClass;
import org.junit.Test;

/**
 * Against a real sshd and wisp, so only when asked (see mobile/README.md):
 *   WISP_SSH_TEST=user@host:sshPort  WISP_SSH_KEY=<private key file>
 *   WISP_SSH_SCRIPT=<bootstrapScript output>  WISP_SSH_PORT=<wisp port there>
 * The phone end listens on [::1] so a wisp on this same computer can keep 127.0.0.1:port.
 */
public class TunnelTest {
    @BeforeClass
    public static void bc() {
        Security.removeProvider("BC");
        Security.insertProviderAt(new BouncyCastleProvider(), 1);
    }

    private static Tunnel tunnel(String keyFile, List<String> lines, Tunnel.HostKeys hk) throws Exception {
        String t = System.getenv("WISP_SSH_TEST");
        String user = t.substring(0, t.indexOf('@'));
        String host = t.substring(t.indexOf('@') + 1, t.lastIndexOf(':'));
        int sshPort = Integer.parseInt(t.substring(t.lastIndexOf(':') + 1));
        Keys keys = new Keys(TestVaults.vault());
        Keys.Info k = keyFile != null ? keys.importKey("test", new String(Files.readAllBytes(Paths.get(keyFile))), null) : keys.create("stranger");
        String script = new String(Files.readAllBytes(Paths.get(System.getenv("WISP_SSH_SCRIPT"))));
        Tunnel tn = new Tunnel(host, sshPort, user, null, Integer.parseInt(System.getenv("WISP_SSH_PORT")), script, keys.load(k.id), hk, lines::add);
        tn.bindHost = "::1";
        return tn;
    }

    private static void waitFor(List<String> lines, String prefix) throws InterruptedException {
        for (int i = 0; i < 300 && lines.stream().noneMatch(l -> l.startsWith(prefix) || l.startsWith("@@wisp fail")); i++) Thread.sleep(100);
    }

    /** Until `count` lines start with `prefix`, or one fails (about 20 s at most). */
    static void waitFor(List<String> lines, String prefix, int count) throws InterruptedException {
        for (int i = 0; i < 200 && lines.stream().filter(l -> l.startsWith(prefix)).count() < count && lines.stream().noneMatch(l -> l.startsWith("@@wisp fail")); i++) Thread.sleep(100);
    }

    private static long count(List<String> lines, String prefix) {
        return lines.stream().filter(l -> l.startsWith(prefix)).count();
    }

    // ---- ADR-0065 edge cases, against FakeSshd (no real sshd needed) ----

    // Scanned from the desktop's QR code, the fingerprint is pinned before the first connect: a
    // computer showing another key is refused during the key exchange, before any password or key.
    @Test
    public void aPinnedFingerprintRefusesAnotherKeyFirstTime() throws Exception {
        try (FakeSshd sshd = new FakeSshd("hunter2", FakeSshd.ec())) {
            String pinned = Keys.fingerprint(FakeSshd.ec().getPublic()); // the QR's key, not this one
            HostTrust.MapKnown known = new HostTrust.MapKnown();
            List<String> lines = new CopyOnWriteArrayList<>();
            Tunnel tn = Reach.tunnel("127.0.0.1", sshd.port(), "me", "hunter2", sshd.wispPort, "#!/bin/sh", FakeSshd.ec(), new HostTrust(known, pinned), lines::add);
            tn.start();
            waitFor(lines, "@@wisp fail");
            assertEquals("failed", tn.state());
            assertTrue(String.join("\n", lines), lines.contains("@@wisp fail ssh " + HostTrust.NOT_PINNED + sshd.fingerprint() + "."));
            assertEquals("no password sent", 0, sshd.passwordTries.get());
            assertEquals("no key offered", 0, sshd.keyTries.get());
            assertNull("the wrong key is not kept", known.get("127.0.0.1:" + sshd.port()));
            tn.stop();

            // The right pin: trusted on the first connect, and kept.
            List<String> ok = new CopyOnWriteArrayList<>();
            Tunnel tn2 = Reach.tunnel("127.0.0.1", sshd.port(), "me", "hunter2", sshd.wispPort, "#!/bin/sh", FakeSshd.ec(), new HostTrust(known, sshd.fingerprint()), ok::add);
            tn2.start();
            waitFor(ok, "@@wisp open", 1);
            assertEquals(String.join("\n", ok), "ready", tn2.state());
            assertEquals(sshd.fingerprint(), known.get("127.0.0.1:" + sshd.port()));
            tn2.stop();
        }
    }

    // A key kept from an earlier connect, and the computer now shows another: refused with
    // *This computer's key changed* and the new fingerprint, nothing sent. Trust the new key (after
    // the page's confirmation) keeps the new one and the next connect goes through.
    @Test
    public void aChangedHostKeyIsRefused() throws Exception {
        try (FakeSshd sshd = new FakeSshd("hunter2", FakeSshd.ec())) {
            HostTrust.MapKnown known = new HostTrust.MapKnown();
            String hostPort = "127.0.0.1:" + sshd.port();
            known.put(hostPort, Keys.fingerprint(FakeSshd.ec().getPublic())); // the old key
            List<String> lines = new CopyOnWriteArrayList<>();
            Tunnel tn = Reach.tunnel("127.0.0.1", sshd.port(), "me", "hunter2", sshd.wispPort, "#!/bin/sh", FakeSshd.ec(), new HostTrust(known, null), lines::add);
            tn.start();
            waitFor(lines, "@@wisp fail");
            assertEquals("failed", tn.state());
            String fail = "@@wisp fail ssh " + HostTrust.CHANGED + sshd.fingerprint() + ".";
            assertTrue(String.join("\n", lines), lines.contains(fail));
            assertEquals(sshd.fingerprint(), HostTrust.changedTo(Reach.failure(fail)));
            assertEquals(0, sshd.passwordTries.get());
            assertEquals(0, sshd.keyTries.get());
            tn.stop();

            // Trusting a fingerprint other than the one shown is refused (a stale confirmation).
            assertFalse(HostTrust.trustNew(known, hostPort, "SHA256:somethingElse", sshd.fingerprint() + "x"));
            assertTrue(HostTrust.trustNew(known, hostPort, sshd.fingerprint(), sshd.fingerprint()));
            List<String> ok = new CopyOnWriteArrayList<>();
            Tunnel tn2 = Reach.tunnel("127.0.0.1", sshd.port(), "me", "hunter2", sshd.wispPort, "#!/bin/sh", FakeSshd.ec(), new HostTrust(known, null), ok::add);
            tn2.start();
            waitFor(ok, "@@wisp open", 1);
            assertEquals(String.join("\n", ok), "ready", tn2.state());
            tn2.stop();
        }
    }

    // SSH stays up but wisp stopped there: the watch misses twice, sshd refuses a channel to wisp's
    // port, so the bootstrap runs again at once and starts wisp. One miss alone restarts nothing.
    @Test
    public void aStoppedWispIsStartedAgainAfterTwoMissedChecks() throws Exception {
        long was = Tunnel.WATCH_MS;
        int wait = Tunnel.WATCH_TIMEOUT_MS;
        Tunnel.WATCH_MS = 400;
        Tunnel.WATCH_TIMEOUT_MS = 500;
        try (FakeSshd sshd = new FakeSshd("hunter2", FakeSshd.ec())) {
            List<String> lines = new CopyOnWriteArrayList<>();
            Tunnel tn = Reach.tunnel("127.0.0.1", sshd.port(), "me", "hunter2", sshd.wispPort, "#!/bin/sh", FakeSshd.ec(), new HostTrust(new HostTrust.MapKnown(), null), lines::add);
            tn.start();
            waitFor(lines, "@@wisp open", 1);
            assertEquals(String.join("\n", lines), "ready", tn.state());
            assertEquals(1, sshd.bootstraps.get());
            assertTrue("the phone end took another port: wisp holds its own here", tn.localPort != sshd.wispPort);
            assertEquals("wlt_test", tn.token);

            long stopped = System.currentTimeMillis();
            sshd.stopWisp();
            waitFor(lines, "@@wisp open", 2);
            long took = System.currentTimeMillis() - stopped;
            assertTrue(String.join("\n", lines), lines.contains("@@wisp lost " + Tunnel.WISP_DOWN));
            assertEquals(2, count(lines, "@@wisp open"));
            assertEquals("the bootstrap ran again", 2, sshd.bootstraps.get());
            assertEquals("and started wisp", 2, sshd.wispStarts.get());
            assertTrue("two checks missed first (" + took + " ms)", took >= 2 * Tunnel.WATCH_MS);
            assertEquals("ready", tn.state());
            tn.stop();
        } finally {
            Tunnel.WATCH_MS = was;
            Tunnel.WATCH_TIMEOUT_MS = wait;
        }
    }

    // A port per computer: two computers connected at once, each its own local port.
    @Test
    public void twoComputersGetTwoLocalPorts() throws Exception {
        try (FakeSshd a = new FakeSshd("pw", FakeSshd.ec()); FakeSshd b = new FakeSshd("pw", FakeSshd.ec())) {
            List<String> la = new CopyOnWriteArrayList<>();
            List<String> lb = new CopyOnWriteArrayList<>();
            Tunnel ta = Reach.tunnel("127.0.0.1", a.port(), "me", "pw", a.wispPort, "#!/bin/sh", FakeSshd.ec(), new HostTrust(new HostTrust.MapKnown(), null), la::add);
            Tunnel tb = Reach.tunnel("127.0.0.1", b.port(), "me", "pw", b.wispPort, "#!/bin/sh", FakeSshd.ec(), new HostTrust(new HostTrust.MapKnown(), null), lb::add);
            ta.start();
            tb.start();
            waitFor(la, "@@wisp open", 1);
            waitFor(lb, "@@wisp open", 1);
            assertEquals("ready", ta.state());
            assertEquals("ready", tb.state());
            assertTrue(ta.localPort > 0 && tb.localPort > 0 && ta.localPort != tb.localPort);
            ta.stop();
            tb.stop();
        }
    }

    @Test
    public void bootstrapsAndForwards() throws Exception {
        assumeTrue(System.getenv("WISP_SSH_TEST") != null);
        List<String> lines = new CopyOnWriteArrayList<>();
        Tunnel tn = tunnel(System.getenv("WISP_SSH_KEY"), lines, (h, p, fp) -> null);
        tn.start();
        waitFor(lines, "@@wisp open");
        System.out.println(String.join("\n", lines));
        assertEquals("ready", tn.state());
        assertTrue(lines.stream().anyMatch(l -> l.startsWith("@@wisp step node done")));
        assertTrue(lines.stream().anyMatch(l -> l.startsWith("@@wisp step wisp done")));
        assertTrue(lines.stream().anyMatch(l -> l.startsWith("@@wisp step tunnel done")));
        // wisp's page answers through the tunnel, with the Host it insists on.
        java.net.HttpURLConnection h = (java.net.HttpURLConnection) new java.net.URL("http://[::1]:" + tn.port + "/").openConnection();
        assertEquals(200, h.getResponseCode());
        assertTrue(new String(h.getInputStream().readAllBytes()).contains("wisp"));

        // A dropped connection comes back on its own.
        tn.close();
        for (int i = 0; i < 100 && lines.stream().filter(l -> l.startsWith("@@wisp open")).count() < 2; i++) Thread.sleep(100);
        assertTrue(String.join("\n", lines), lines.stream().anyMatch(l -> l.startsWith("@@wisp lost")));
        assertTrue(lines.contains("@@wisp step ssh run Reconnecting…"));
        assertEquals(2, lines.stream().filter(l -> l.startsWith("@@wisp open")).count());
        assertEquals("ready", tn.state());

        // Back in front with a live tunnel: nudge checks it through wisp and leaves it up.
        tn.nudge();
        Thread.sleep(2000);
        assertEquals(2, lines.stream().filter(l -> l.startsWith("@@wisp open")).count());
        assertEquals("ready", tn.state());
        tn.stop();
        assertEquals("idle", tn.state());
    }

    @Test
    public void startsWispAgainWhenItStops() throws Exception {
        assumeTrue(System.getenv("WISP_SSH_TEST") != null);
        Tunnel.WATCH_MS = 1_000;
        List<String> lines = new CopyOnWriteArrayList<>();
        Tunnel tn = tunnel(System.getenv("WISP_SSH_KEY"), lines, (h, p, fp) -> null);
        tn.start();
        waitFor(lines, "@@wisp open");
        assertEquals("ready", tn.state());
        // wisp quits there; SSH stays up (the script's `cat` holds the session).
        String ready = lines.stream().filter(l -> l.startsWith("@@wisp ready")).findFirst().get();
        new ProcessBuilder("kill", ready.substring(ready.lastIndexOf(' ') + 1)).start().waitFor();
        for (int i = 0; i < 600 && lines.stream().filter(l -> l.startsWith("@@wisp open")).count() < 2; i++) Thread.sleep(100);
        System.out.println(String.join("\n", lines));
        assertTrue(lines.contains("@@wisp lost " + Tunnel.WISP_DOWN));
        assertTrue(lines.stream().anyMatch(l -> l.startsWith("@@wisp step wisp done started")));
        assertEquals(2, lines.stream().filter(l -> l.startsWith("@@wisp open")).count());
        assertEquals("ready", tn.state());
        tn.stop();
        Tunnel.WATCH_MS = 15_000;
    }

    @Test
    public void unknownKeyAndChangedHostKeyFail() throws Exception {
        assumeTrue(System.getenv("WISP_SSH_TEST") != null);
        List<String> lines = new CopyOnWriteArrayList<>();
        Tunnel tn = tunnel(null, lines, (h, p, fp) -> null);
        tn.start();
        waitFor(lines, "@@wisp fail");
        assertEquals("failed", tn.state());
        assertTrue(String.join("\n", lines), lines.stream().anyMatch(l -> l.startsWith("@@wisp fail ssh Authentication failed")));

        List<String> lines2 = new CopyOnWriteArrayList<>();
        Tunnel tn2 = tunnel(System.getenv("WISP_SSH_KEY"), lines2, (h, p, fp) -> "This computer’s key changed: it is now " + fp + ".");
        tn2.start();
        waitFor(lines2, "@@wisp fail");
        assertTrue(String.join("\n", lines2), lines2.stream().anyMatch(l -> l.startsWith("@@wisp fail ssh This computer’s key changed: it is now SHA256:")));
    }
}
