package com.bong.client.combat;

import com.bong.client.BongClient;
import com.bong.client.hud.HudImmersionMode;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;

/**
 * Wires the combat-HUD stores, key-binding dispatch, and on-disconnect reset
 * (§8.3). Dispatch of the resulting intents into the real network protocol is
 * intentionally left as a TODO until the server-side handlers land — the
 * client-side HUD state already transitions correctly for local-feedback
 * rendering.
 */
public final class CombatHudBootstrap {
    static final int ZHENMAI_PREP_WINDOW_MS = 1000;

    private CombatHudBootstrap() {
    }

    public static void register() {
        CombatKeybindings.register();
        CombatKeybindings.setQuickSlotHandler(CombatHudBootstrap::onQuickSlotPressed);
        CombatKeybindings.setJiemaiHandler(CombatHudBootstrap::onJiemaiPressed);
        CombatKeybindings.setSpellVolumeHoldHandler(CombatHudBootstrap::onSpellVolumeHold);
        CombatKeybindings.setEventStreamToggleHandler(CombatHudBootstrap::onEventStreamToggle);

        ClientPlayConnectionEvents.DISCONNECT.register((handler, client) -> client.execute(CombatHudBootstrap::resetOnDisconnect));
        BongClient.LOGGER.info("Combat HUD bootstrap ready.");
    }

    private static void onQuickSlotPressed(int slot) {
        long now = System.currentTimeMillis();
        CastState current = CastStateStore.snapshot();
        if (current.isCasting()) {
            if (current.slot() == slot) {
                // §4.2: repeating the same F key during cast is ignored (audio feedback hook).
                return;
            }
            // Different F key — cancel current cast.
            CastStateStore.interrupt(CastOutcome.USER_CANCEL, now);
        }
        QuickSlotEntry entry = QuickUseSlotStore.snapshot().slot(slot);
        if (entry == null) return;

        // Predict the cast-begin client-side so the UI responds instantly.
        CastStateStore.beginCast(slot, entry.castDurationMs(), now);
        com.bong.client.network.ClientRequestSender.sendUseQuickSlot(slot);
    }

    private static void onJiemaiPressed() {
        long now = System.currentTimeMillis();
        DefenseWindowStore.open(ZHENMAI_PREP_WINDOW_MS, now);
        com.bong.client.network.ClientRequestSender.sendJiemai();
    }

    private static void onSpellVolumeHold(boolean pressed) {
        SpellVolumeState current = SpellVolumeStore.snapshot();
        if (pressed) {
            SpellVolumeStore.show(current.radius(), current.velocityCap(), current.qiInvest());
        } else {
            SpellVolumeStore.hide();
        }
    }

    private static void onEventStreamToggle() {
        boolean visible = HudConfig.toggleEventStreamVisible();
        BongClient.LOGGER.info("Combat event stream HUD {}", visible ? "shown" : "hidden");
    }

    static void resetOnDisconnect() {
        // §8.3 hydration expects a fresh first-frame payload post-reconnect.
        CombatHudStateStore.resetForTests();
        CastStateStore.resetForTests();
        DefenseWindowStore.resetForTests();
        QuickUseSlotStore.resetForTests();
        SkillBarStore.resetForTests();
        UnlockedStylesStore.resetForTests();
        UnifiedEventStore.resetForTests();
        SpellVolumeStore.resetForTests();
        // Combat UI stores (plan-combat-ui).
        com.bong.client.combat.store.WoundsStore.resetForTests();
        com.bong.client.combat.store.StatusEffectStore.resetForTests();
        com.bong.client.combat.store.DerivedAttrsStore.resetForTests();
        com.bong.client.combat.store.DamageFloaterStore.resetForTests();
        com.bong.client.combat.store.DeathStateStore.resetForTests();
        com.bong.client.combat.store.TerminateStateStore.resetForTests();
        com.bong.client.combat.store.TribulationStateStore.resetForTests();
        com.bong.client.combat.store.TribulationBroadcastStore.resetForTests();
        com.bong.client.combat.store.CarrierStateStore.resetForTests();
        com.bong.client.combat.inspect.TechniquesListPanel.resetForTests();
        com.bong.client.combat.inspect.WeaponTreasurePanel.resetForTests();
        com.bong.client.social.SocialStateStore.clearOnDisconnect();
        TreasureEquippedStore.resetForTests();
        HudImmersionMode.resetForTests();
    }
}
