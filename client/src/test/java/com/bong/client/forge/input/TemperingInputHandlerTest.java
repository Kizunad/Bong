package com.bong.client.forge.input;

import com.bong.client.forge.ForgeScreen;
import com.bong.client.forge.state.ForgeSessionStore;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import org.lwjgl.glfw.GLFW;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class TemperingInputHandlerTest {
    private record Sent(Identifier channel, String body) {}

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        ForgeSessionStore.resetForTests();
    }

    private void install() {
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8)))
        );
    }

    @Test
    void j_key_emits_light_hit_when_screen_open_and_step_tempering() {
        install();
        boolean handled = TemperingInputHandler.handleKey(
            new ForgeScreen(),
            GLFW.GLFW_KEY_J,
            snapshot("tempering")
        );

        assertTrue(handled);
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"forge_tempering_hit\",\"v\":1,\"session_id\":7,\"beat\":\"L\",\"ticks_remaining\":1}",
            sent.get(0).body()
        );
    }

    @Test
    void j_key_ignored_when_screen_closed() {
        install();
        assertFalse(TemperingInputHandler.handleKey(null, GLFW.GLFW_KEY_J, snapshot("tempering")));
        assertTrue(sent.isEmpty());
    }

    @Test
    void j_key_ignored_when_step_billet() {
        install();
        assertFalse(TemperingInputHandler.handleKey(new ForgeScreen(), GLFW.GLFW_KEY_J, snapshot("billet")));
        assertTrue(sent.isEmpty());
    }

    @Test
    void k_key_emits_heavy_hit() {
        install();
        assertTrue(TemperingInputHandler.handleKey(new ForgeScreen(), GLFW.GLFW_KEY_K, snapshot("tempering")));
        assertEquals(
            "{\"type\":\"forge_tempering_hit\",\"v\":1,\"session_id\":7,\"beat\":\"H\",\"ticks_remaining\":1}",
            sent.get(0).body()
        );
    }

    @Test
    void l_key_emits_fold_hit() {
        install();
        assertTrue(TemperingInputHandler.handleKey(new ForgeScreen(), GLFW.GLFW_KEY_L, snapshot("tempering")));
        assertEquals(
            "{\"type\":\"forge_tempering_hit\",\"v\":1,\"session_id\":7,\"beat\":\"F\",\"ticks_remaining\":1}",
            sent.get(0).body()
        );
    }

    private static ForgeSessionStore.Snapshot snapshot(String step) {
        return new ForgeSessionStore.Snapshot(
            7,
            "qing_feng_v0",
            "青锋剑",
            true,
            step,
            1,
            2,
            "{\"step\":\"" + step + "\"}"
        );
    }
}
