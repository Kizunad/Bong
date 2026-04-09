package com.bong.client;

import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;
import net.minecraft.util.Formatting;

public final class BongServerPayloadRouter {
    private BongServerPayloadRouter() {
    }

    public static boolean route(MinecraftClient client, BongServerPayload payload) {
        switch (payload.kind()) {
            case WELCOME -> handleWelcome((BongServerPayload.WelcomePayload) payload);
            case HEARTBEAT -> handleHeartbeat((BongServerPayload.HeartbeatPayload) payload);
            case NARRATION -> handleNarration(client, (BongServerPayload.NarrationPayload) payload);
            case ZONE_INFO -> handleZoneInfo((BongServerPayload.ZoneInfoPayload) payload);
            case EVENT_ALERT -> handleEventAlert((BongServerPayload.EventAlertPayload) payload);
            case PLAYER_STATE -> handlePlayerState((BongServerPayload.PlayerStatePayload) payload);
        }

        return true;
    }

    private static void handleWelcome(BongServerPayload.WelcomePayload payload) {
        BongClient.LOGGER.info("Received welcome payload: {}", payload.message());
    }

    private static void handleHeartbeat(BongServerPayload.HeartbeatPayload payload) {
        BongClient.LOGGER.debug("Received heartbeat payload: {}", payload.message());
    }

    private static void handleNarration(MinecraftClient client, BongServerPayload.NarrationPayload payload) {
        BongClient.LOGGER.debug("Received narration payload with {} narration entries", payload.narrations().size());

        NarrationState.ChatSink chatSink = snapshot -> sendNarrationToChat(client, snapshot);
        for (BongServerPayload.Narration narration : payload.narrations()) {
            NarrationState.recordNarration(narration, System.currentTimeMillis(), chatSink);
        }
    }

    private static void sendNarrationToChat(MinecraftClient client, NarrationState.NarrationSnapshot snapshot) {
        if (client == null || client.player == null) {
            return;
        }

        client.player.sendMessage(
                Text.literal(snapshot.chatLine()).formatted(chatFormatting(snapshot.style())),
                false
        );
    }

    private static Formatting chatFormatting(String style) {
        return switch (style) {
            case "system_warning" -> Formatting.RED;
            case "perception" -> Formatting.GRAY;
            case "era_decree" -> Formatting.GOLD;
            default -> Formatting.WHITE;
        };
    }

    private static void handleZoneInfo(BongServerPayload.ZoneInfoPayload payload) {
        ZoneInfoHandler.handle(payload);
        BongClient.LOGGER.debug(
                "Received zone_info payload for zone={} dangerLevel={}",
                payload.zoneInfo().zone(),
                payload.zoneInfo().dangerLevel()
        );
    }

    private static void handleEventAlert(BongServerPayload.EventAlertPayload payload) {
        EventAlertHandler.handle(payload);
        BongClient.LOGGER.info(
                "Received event_alert payload kind={} severity={}",
                payload.eventAlert().kind(),
                payload.eventAlert().severity()
        );
    }

    private static void handlePlayerState(BongServerPayload.PlayerStatePayload payload) {
        PlayerStateHandler.handle(payload);
        BongClient.LOGGER.debug(
                "Received player_state payload realm={} zone={} compositePower={}",
                payload.playerState().realm(),
                payload.playerState().zone(),
                payload.playerState().compositePower()
        );
    }
}
