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
        // plan-shield-block-v1 P1 — 举盾键盘备用路径（默认 UNKNOWN，主路径为右键 MixinMouse）。
        CombatKeybindings.setShieldHoldHandler(CombatHudBootstrap::onShieldHold);

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

    /**
     * interaction-intent-cleanup-v1 P3 — 截脉窗口守卫。
     *
     * <p>截脉是「反应技」：只在 server 推送的防御窗口（{@link DefenseWindowStore} active）
     * 内有效。此前无守卫——任意时刻按 V 都会 {@code open(...)} 本地伪造一个截脉环 + 发出
     * 无用的 {@code jiemai} C2S（server 醒灵境门控只回 debug 日志，玩家只看到 HUD 幻像）。
     * 加守卫后：窗口未开 → 直接返回，既不发包也不开环；窗口开启 → 维持原行为（刷新本地窗口
     * 计时供 HUD 渲染 + 发包）。
     *
     * <p>包级可见以便单测直接驱动（同 {@link #resetOnDisconnect()}）。
     */
    static void onJiemaiPressed() {
        if (!DefenseWindowStore.snapshot().active()) {
            // 窗口未开：无截脉机会，吞掉按键——不发无用 C2S、不点亮假截脉环。
            return;
        }
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

    // plan-shield-block-v1 P1 — 键盘备用举盾路径（默认 UNKNOWN，实际触发在 MixinMouse 右键路径）。
    private static void onShieldHold(boolean pressed) {
        if (pressed) {
            com.bong.client.network.ClientRequestSender.sendRaiseShield();
        } else {
            com.bong.client.network.ClientRequestSender.sendLowerShield();
        }
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
