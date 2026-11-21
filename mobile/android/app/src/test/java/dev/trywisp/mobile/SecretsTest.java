package dev.trywisp.mobile;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;

import java.nio.charset.StandardCharsets;
import org.junit.Test;

/** The page's SecureStore: what wisp Cloud's sign-in seals its device token in (noVault without it). */
public class SecretsTest {
    @Test
    public void keepsReadsAndForgetsAValue() throws Exception {
        Secrets s = new Secrets(TestVaults.vault());
        assertNull(s.get("wisp.cloud.token:grid.trywisp.dev/mo"));
        s.set("wisp.cloud.token:grid.trywisp.dev/mo", "tok-1");
        assertEquals("tok-1", s.get("wisp.cloud.token:grid.trywisp.dev/mo"));
        s.set("wisp.cloud.token:grid.trywisp.dev/mo", "tok-2");
        assertEquals("tok-2", s.get("wisp.cloud.token:grid.trywisp.dev/mo"));
        s.remove("wisp.cloud.token:grid.trywisp.dev/mo");
        assertNull(s.get("wisp.cloud.token:grid.trywisp.dev/mo"));
    }

    @Test
    public void sealsOnDiskAndStaysApartFromKeysAndComputerTokens() throws Exception {
        TestVaults.MapShelf shelf = new TestVaults.MapShelf();
        SealedVault vault = TestVaults.vault(shelf);
        vault.put(Computers.TOKEN + "c1", "computer".getBytes(StandardCharsets.UTF_8));
        Secrets s = new Secrets(vault);
        s.set("token/c1", "page");
        // the page can't reach the computer's token through its own key
        assertEquals("computer", new String(vault.get(Computers.TOKEN + "c1"), StandardCharsets.UTF_8));
        assertEquals("page", s.get("token/c1"));
        for (String v : shelf.map.values()) assertFalse(v.contains("page"));
        assertTrue(shelf.map.keySet().stream().anyMatch(k -> k.contains(Secrets.PREFIX + "token/c1")));
    }

    @Test(expected = IllegalArgumentException.class)
    public void refusesAnEmptyKey() throws Exception {
        new Secrets(TestVaults.vault()).set("", "x");
    }
}
