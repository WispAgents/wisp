package dev.trywisp.mobile;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import java.util.List;
import org.json.JSONObject;
import org.junit.Test;

public class StayTest {
    private static JSONObject c(String kind, boolean push) throws Exception {
        return new JSONObject().put("id", kind).put("kind", kind).put("push", push);
    }

    // ADR-0065 §3.6: the foreground service runs only while an SSH computer without push is
    // connected, and only with the user's Stay connected switch (v1 ran it whenever connected).
    @Test
    public void theServiceRunsOnlyForAnSshComputerWithoutPushAndTheSwitchOn() throws Exception {
        assertTrue(Stay.needed(true, List.of(c("ssh", false))));
        assertFalse("the switch is off", Stay.needed(false, List.of(c("ssh", false))));
        assertFalse("push carries the alerts", Stay.needed(true, List.of(c("ssh", true))));
        assertFalse("a grid computer has no tunnel to hold", Stay.needed(true, List.of(c("grid", false))));
        assertFalse("nothing connected", Stay.needed(true, List.of()));
        assertTrue("one of several is enough", Stay.needed(true, List.of(c("grid", false), c("ssh", true), c("ssh", false))));
        assertFalse("a forgotten computer (null) counts for nothing", Stay.needed(true, java.util.Arrays.asList((JSONObject) null)));
    }
}
