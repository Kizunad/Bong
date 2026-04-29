package com.bong.client.social;

import com.bong.client.network.ClientRequestSender;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;

public final class SparringInviteScreenBootstrap {
    private SparringInviteScreenBootstrap() {
    }

    public static void register() {
        ClientTickEvents.END_CLIENT_TICK.register(SparringInviteScreenBootstrap::onEndClientTick);
    }

    static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) return;
        SocialStateStore.SparringInvite invite = SocialStateStore.sparringInvite();
        Screen current = client.currentScreen;
        if (invite == null) {
            if (current instanceof SparringInviteScreen) {
                client.setScreen(null);
            }
            return;
        }
        if (invite.expiresAtMs() <= System.currentTimeMillis()) {
            ClientRequestSender.sendSparringInviteResponse(invite.inviteId(), false, true);
            SocialStateStore.clearSparringInvite(invite.inviteId());
            if (current instanceof SparringInviteScreen) {
                client.setScreen(null);
            }
            return;
        }
        if (!(current instanceof SparringInviteScreen screen)
            || !screen.inviteIdForTests().equals(invite.inviteId())) {
            client.setScreen(new SparringInviteScreen(invite));
        }
    }
}
