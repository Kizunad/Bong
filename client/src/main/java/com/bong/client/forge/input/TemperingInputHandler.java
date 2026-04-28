package com.bong.client.forge.input;

import com.bong.client.forge.ForgeScreen;
import com.bong.client.forge.state.ForgeSessionStore;
import com.bong.client.hud.BongToast;
import com.bong.client.network.ClientRequestProtocol;
import com.bong.client.network.ClientRequestSender;
import org.lwjgl.glfw.GLFW;

/** plan-forge-leftovers-v1 §3.2 — ForgeScreen 内 J/K/L 淬炼击键输入。 */
public final class TemperingInputHandler {
    private static final long TOAST_MS = 700L;
    private static final int LIGHT_COLOR = 0xFF49A7FF;
    private static final int HEAVY_COLOR = 0xFFFF6666;
    private static final int FOLD_COLOR = 0xFFFFD45A;

    private TemperingInputHandler() {}

    public static boolean handleKey(Object screen, int keyCode) {
        return handleKey(screen, keyCode, ForgeSessionStore.snapshot());
    }

    static boolean handleKey(Object screen, int keyCode, ForgeSessionStore.Snapshot snapshot) {
        if (!(screen instanceof ForgeScreen) || snapshot == null || snapshot.sessionId() <= 0) {
            return false;
        }
        if (!"tempering".equals(snapshot.currentStep())) {
            return false;
        }

        ClientRequestProtocol.TemperBeat beat = beatForKey(keyCode);
        if (beat == null) {
            return false;
        }

        onTemperingKey(beat, snapshot.sessionId());
        return true;
    }

    private static void onTemperingKey(ClientRequestProtocol.TemperBeat beat, long sessionId) {
        ClientRequestSender.sendForgeTemperingHit(sessionId, beat, ticksRemainingForClientHit());
        showHitToast(beat);
    }

    static ClientRequestProtocol.TemperBeat beatForKey(int keyCode) {
        return switch (keyCode) {
            case GLFW.GLFW_KEY_J -> ClientRequestProtocol.TemperBeat.L;
            case GLFW.GLFW_KEY_K -> ClientRequestProtocol.TemperBeat.H;
            case GLFW.GLFW_KEY_L -> ClientRequestProtocol.TemperBeat.F;
            default -> null;
        };
    }

    static int ticksRemainingForClientHit() {
        return 1;
    }

    private static void showHitToast(ClientRequestProtocol.TemperBeat beat) {
        BongToast.show(toastText(beat), toastColor(beat), System.currentTimeMillis(), TOAST_MS);
    }

    static String toastText(ClientRequestProtocol.TemperBeat beat) {
        return switch (beat) {
            case L -> "Light hit";
            case H -> "Heavy hit";
            case F -> "Fold hit";
        };
    }

    static int toastColor(ClientRequestProtocol.TemperBeat beat) {
        return switch (beat) {
            case L -> LIGHT_COLOR;
            case H -> HEAVY_COLOR;
            case F -> FOLD_COLOR;
        };
    }
}
