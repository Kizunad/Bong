package com.bong.client.input;

import org.junit.jupiter.api.Test;
import org.lwjgl.glfw.GLFW;

import static org.junit.jupiter.api.Assertions.assertEquals;

public class InteractionKeybindingsTest {
    @Test
    void defaultInteractKeyIsG() {
        assertEquals(GLFW.GLFW_KEY_G, InteractionKeybindings.DEFAULT_KEY_CODE);
        assertEquals("key.bong-client.interact", InteractionKeybindings.INTERACT_KEY_TRANSLATION);
    }
}
