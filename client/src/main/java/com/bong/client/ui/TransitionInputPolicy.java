package com.bong.client.ui;

import org.lwjgl.glfw.GLFW;

public final class TransitionInputPolicy {
    private TransitionInputPolicy() {
    }

    public static boolean shouldBlockMouse(boolean inputLocked) {
        return inputLocked;
    }

    public static KeyDecision keyDecision(boolean inputLocked, int keyCode, int action) {
        if (!inputLocked || action != GLFW.GLFW_PRESS) {
            return KeyDecision.PASS;
        }
        if (keyCode == GLFW.GLFW_KEY_ESCAPE) {
            return KeyDecision.CANCEL_AND_CLOSE;
        }
        return KeyDecision.CONSUME;
    }

    public enum KeyDecision {
        PASS,
        CONSUME,
        CANCEL_AND_CLOSE
    }
}
