package dev.trywisp.mobile;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.PublicKey;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicInteger;
import org.apache.sshd.common.keyprovider.KeyPairProvider;
import org.apache.sshd.server.Environment;
import org.apache.sshd.server.ExitCallback;
import org.apache.sshd.server.SshServer;
import org.apache.sshd.server.channel.ChannelSession;
import org.apache.sshd.server.command.Command;
import org.apache.sshd.server.forward.AcceptAllForwardingFilter;

/**
 * An sshd in the test's JVM, standing in for a computer: a password, the keys `ADD_KEY` added, a
 * bootstrap (`sh -s`) that starts a fake wisp when it isn't running and says `@@wisp ready`, the
 * server.json read, and direct-tcpip to the fake wisp's port. It counts every password and key
 * offered, so a test can say nothing was sent.
 */
final class FakeSshd implements AutoCloseable {
    final SshServer sshd = SshServer.setUpDefaultServer();
    final AtomicInteger passwordTries = new AtomicInteger();
    final AtomicInteger keyTries = new AtomicInteger();
    final AtomicInteger bootstraps = new AtomicInteger();
    final AtomicInteger wispStarts = new AtomicInteger();
    final Set<String> authorized = ConcurrentHashMap.newKeySet();
    final String password;
    final KeyPair hostKey;
    final int wispPort;
    private volatile ServerSocket wisp;

    FakeSshd(String password, KeyPair hostKey) throws IOException {
        this.password = password;
        this.hostKey = hostKey;
        try (ServerSocket s = new ServerSocket(0, 1, java.net.InetAddress.getLoopbackAddress())) {
            wispPort = s.getLocalPort();
        }
        sshd.setHost("127.0.0.1");
        sshd.setPort(0);
        sshd.setKeyPairProvider(KeyPairProvider.wrap(hostKey));
        sshd.setPasswordAuthenticator((user, pw, session) -> {
            passwordTries.incrementAndGet();
            return password != null && password.equals(pw);
        });
        sshd.setKeyboardInteractiveAuthenticator(null);
        sshd.setPublickeyAuthenticator((user, key, session) -> {
            keyTries.incrementAndGet();
            return authorized.contains(line(key));
        });
        sshd.setForwardingFilter(AcceptAllForwardingFilter.INSTANCE);
        sshd.setCommandFactory((channel, command) -> new Cmd(command));
        sshd.start();
    }

    int port() {
        return sshd.getPort();
    }

    static KeyPair ec() throws Exception {
        KeyPairGenerator g = KeyPairGenerator.getInstance("EC");
        g.initialize(256);
        return g.generateKeyPair();
    }

    static String line(PublicKey k) {
        String[] parts = Keys.authorizedKey(k).split(" ");
        return parts[0] + " " + parts[1];
    }

    String fingerprint() {
        return Keys.fingerprint(hostKey.getPublic());
    }

    boolean wispUp() {
        return wisp != null;
    }

    /** wisp quits there; SSH stays up. */
    void stopWisp() throws IOException {
        ServerSocket s = wisp;
        wisp = null;
        if (s != null) s.close();
    }

    private synchronized void startWisp() throws IOException {
        if (wisp != null) return;
        ServerSocket s = new ServerSocket();
        s.setReuseAddress(true);
        s.bind(new InetSocketAddress("127.0.0.1", wispPort));
        wisp = s;
        wispStarts.incrementAndGet();
        Thread t = new Thread(() -> {
            while (!s.isClosed()) {
                try (Socket c = s.accept()) {
                    BufferedReader r = new BufferedReader(new InputStreamReader(c.getInputStream(), StandardCharsets.UTF_8));
                    for (String l; (l = r.readLine()) != null && !l.isEmpty();) {
                        // headers
                    }
                    byte[] body = "{\"version\":\"2.0.0-test\"}".getBytes(StandardCharsets.UTF_8);
                    OutputStream o = c.getOutputStream();
                    o.write(("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: " + body.length + "\r\nConnection: close\r\n\r\n").getBytes(StandardCharsets.UTF_8));
                    o.write(body);
                    o.flush();
                } catch (IOException e) {
                    // closed: wisp stopped
                }
            }
        }, "fake-wisp");
        t.setDaemon(true);
        t.start();
    }

    @Override
    public void close() throws IOException {
        stopWisp();
        sshd.stop(true);
    }

    private final class Cmd implements Command {
        private final String command;
        private InputStream in;
        private OutputStream out;
        private ExitCallback exit;
        private Thread thread;

        Cmd(String command) {
            this.command = command;
        }

        @Override
        public void setInputStream(InputStream in) {
            this.in = in;
        }

        @Override
        public void setOutputStream(OutputStream out) {
            this.out = out;
        }

        @Override
        public void setErrorStream(OutputStream err) {}

        @Override
        public void setExitCallback(ExitCallback callback) {
            this.exit = callback;
        }

        @Override
        public void start(ChannelSession channel, Environment env) {
            thread = new Thread(this::run, "fake-cmd");
            thread.setDaemon(true);
            thread.start();
        }

        private void say(String s) throws IOException {
            out.write((s + "\n").getBytes(StandardCharsets.UTF_8));
            out.flush();
        }

        private void run() {
            int code = 0;
            try {
                if (command.equals(Tunnel.ADD_KEY)) {
                    String k = new BufferedReader(new InputStreamReader(in, StandardCharsets.UTF_8)).readLine();
                    if (k != null) {
                        String[] p = k.split(" ");
                        authorized.add(p[0] + " " + p[1]);
                    }
                } else if (command.contains("server.json")) {
                    say("{\"token\":\"wlt_test\",\"peer\":\"studio-1f3a\"}");
                } else if (command.equals("sh -s")) {
                    bootstraps.incrementAndGet();
                    say("@@wisp step ssh done");
                    if (wisp == null) {
                        startWisp();
                        say("@@wisp step wisp done started");
                    } else {
                        say("@@wisp step wisp done running");
                    }
                    say("@@wisp ready 4242");
                    byte[] buf = new byte[4096];
                    while (in.read(buf) >= 0) {
                        // the script's `cat` holds the session
                    }
                } else {
                    code = 127;
                }
            } catch (IOException e) {
                code = 1;
            }
            exit.onExit(code);
        }

        @Override
        public void destroy(ChannelSession channel) {
            if (thread != null) thread.interrupt();
        }
    }
}
