package com.bong.client.insight;

import com.bong.client.BongClient;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.sound.SoundCategory;
import net.minecraft.sound.SoundEvents;

import java.util.Objects;
import java.util.function.Consumer;
import java.util.function.LongSupplier;

/**
 * 监听 {@link InsightOfferStore}：新邀约到达时开屏，当前邀约结算时关闭该屏。
 */
public final class InsightOfferScreenBootstrap {
    private InsightOfferScreenBootstrap() {
    }

    public static void register() {
        InsightOfferStore.addListener(InsightOfferScreenBootstrap::onStoreChanged);
        BongClient.LOGGER.info("Registered insight offer screen bootstrap via store listener");
    }

    static void onStoreChanged(InsightOfferViewModel offer) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null) {
            return;
        }
        client.execute(() -> applyStoreChange(client, offer));
    }

    static void applyStoreChange(MinecraftClient client, InsightOfferViewModel offer) {
        if (applyStoreChange(client.currentScreen, client::setScreen, offer, System::currentTimeMillis)) {
            playOpenSound(client);
        }
    }

    static boolean applyStoreChange(
        Screen current,
        Consumer<Screen> setScreen,
        InsightOfferViewModel offer,
        LongSupplier clock
    ) {
        Objects.requireNonNull(setScreen, "setScreen");
        Objects.requireNonNull(clock, "clock");
        if (offer != null && InsightOfferStore.snapshot() != offer) {
            return false;
        }
        if (offer == null) {
            if (current instanceof InsightOfferScreen) {
                setScreen.accept(null);
            }
            return false;
        }
        if (offer.isExpired(clock.getAsLong())) {
            InsightOfferStore.settle(
                offer,
                InsightOfferStore.TerminalCause.TIMEOUT,
                InsightDecision.timedOut(offer.triggerId())
            );
            return false;
        }
        Throwable failure = null;
        if (current instanceof InsightOfferScreen existing) {
            if (existing.offer().triggerId().equals(offer.triggerId())) {
                return false;
            }
            try {
                existing.settleForReplacement();
            } catch (Throwable exception) {
                failure = exception;
            }
        }
        try {
            setScreen.accept(new InsightOfferScreen(offer));
        } catch (Throwable exception) {
            if (failure == null) {
                failure = exception;
            } else {
                failure.addSuppressed(exception);
            }
        }
        if (failure != null) {
            rethrow(failure);
        }
        return true;
    }

    private static void rethrow(Throwable failure) {
        if (failure instanceof RuntimeException exception) {
            throw exception;
        }
        if (failure instanceof Error error) {
            throw error;
        }
        throw new AssertionError("insight replacement failed", failure);
    }

    static void playOpenSound(MinecraftClient client) {
        if (client.player != null) {
            client.player.playSound(SoundEvents.BLOCK_BEACON_ACTIVATE, SoundCategory.PLAYERS, 0.4F, 1.0F);
            client.player.playSound(SoundEvents.ENTITY_PLAYER_LEVELUP, SoundCategory.PLAYERS, 0.3F, 1.2F);
        }
    }
}
