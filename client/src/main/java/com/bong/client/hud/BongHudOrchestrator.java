package com.bong.client.hud;

import com.bong.client.BongClientFeatures;

import java.util.ArrayList;
import java.util.List;

public final class BongHudOrchestrator {
    public static final String BASELINE_LABEL = "Bong Client Connected";

    private static final int BASELINE_X = 10;
    private static final int BASELINE_Y = 10;
    private static final int LINE_HEIGHT = 12;
    private static final int DEFAULT_TEXT_WIDTH = 220;

    private BongHudOrchestrator() {
    }

    public static List<HudRenderCommand> buildCommands(
        BongHudStateSnapshot snapshot,
        long nowMillis,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxTextWidth
    ) {
        return buildCommands(snapshot, nowMillis, widthMeasurer, maxTextWidth, 0, 0);
    }

    public static List<HudRenderCommand> buildCommands(
        BongHudStateSnapshot snapshot,
        long nowMillis,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxTextWidth,
        int screenWidth,
        int screenHeight
    ) {
        return buildCommands(
            snapshot,
            CombatHudSnapshot.empty(),
            nowMillis,
            widthMeasurer,
            maxTextWidth,
            screenWidth,
            screenHeight
        );
    }

    public static List<HudRenderCommand> buildCommands(
        BongHudStateSnapshot snapshot,
        CombatHudSnapshot combat,
        long nowMillis,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxTextWidth,
        int screenWidth,
        int screenHeight
    ) {
        return buildCommands(snapshot, combat, nowMillis, widthMeasurer, maxTextWidth, screenWidth, screenHeight, null);
    }

    public static List<HudRenderCommand> buildCommands(
        BongHudStateSnapshot snapshot,
        CombatHudSnapshot combat,
        long nowMillis,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxTextWidth,
        int screenWidth,
        int screenHeight,
        BotanyProjection.Anchor botanyAnchor
    ) {
        BongHudStateSnapshot safeSnapshot = snapshot == null ? BongHudStateSnapshot.empty() : snapshot;
        CombatHudSnapshot combatSnapshot = combat == null ? CombatHudSnapshot.empty() : combat;
        int normalizedWidth = normalizeWidth(maxTextWidth);
        List<HudRenderCommand> commands = new ArrayList<>();
        commands.add(HudRenderCommand.text(HudRenderLayer.BASELINE, BASELINE_LABEL, BASELINE_X, BASELINE_Y, 0xFFFFFF));

        int nextY = BASELINE_Y + LINE_HEIGHT;
        if (ZoneHudRenderer.append(
            commands,
            safeSnapshot.zoneState(),
            nowMillis,
            widthMeasurer,
            normalizedWidth,
            BASELINE_X,
            nextY,
            screenWidth,
            screenHeight
        )) {
            nextY += LINE_HEIGHT;
        }

        if (BongClientFeatures.ENABLE_TOASTS
            && ToastHudRenderer.append(commands, nowMillis, widthMeasurer, normalizedWidth, BASELINE_X, nextY)) {
            nextY += LINE_HEIGHT;
        }

        commands.addAll(OverweightHudPlanner.buildCommands(widthMeasurer, normalizedWidth));
        // 地面 dropped loot 改走 world-space billboard（DroppedItemWorldRenderer），
        // HUD marker 路径已下线——两套定位系统并存会让文字标签相对图标"乱飘"。

        if (BongClientFeatures.ENABLE_VISUAL_EFFECTS) {
            com.bong.client.visual.realm_vision.RealmVisionCommand realmVisionCommand =
                com.bong.client.visual.realm_vision.RealmVisionPlanner.plan(
                    com.bong.client.visual.realm_vision.RealmVisionStateStore.snapshot(),
                    nowMillis / 50L
                );
            com.bong.client.visual.realm_vision.RealmVisionTintRenderer.append(commands, realmVisionCommand);
            VisualHudRenderer.append(
                commands,
                safeSnapshot.visualEffectState(),
                nowMillis,
                widthMeasurer,
                normalizedWidth,
                screenWidth,
                screenHeight
            );
        }

        if (BongClientFeatures.ENABLE_COMBAT_HUD) {
            commands.addAll(MiniBodyHudPlanner.buildCommands(
                combatSnapshot.combatHudState(),
                combatSnapshot.physicalBody(),
                com.bong.client.inventory.state.InventoryStateStore.snapshot().equipped(),
                nowMillis,
                screenWidth,
                screenHeight
            ));
            commands.addAll(QuickBarHudPlanner.buildCommands(
                combatSnapshot.quickSlotConfig(),
                combatSnapshot.skillBarConfig(),
                combatSnapshot.selectedHotbarSlot(),
                combatSnapshot.castState(),
                com.bong.client.inventory.state.InventoryStateStore.snapshot().hotbar(),
                nowMillis,
                screenWidth,
                screenHeight
            ));
            // plan-weapon-v1 §4.3：武器槽贴 hotbar 左右两端。
            commands.addAll(WeaponHotbarHudPlanner.buildCommands(screenWidth, screenHeight));
            commands.addAll(EventStreamHudPlanner.buildCommands(
                combatSnapshot.eventStream(),
                nowMillis,
                widthMeasurer,
                screenWidth,
                screenHeight
            ));
            commands.addAll(JiemaiRingHudPlanner.buildCommands(
                combatSnapshot.defenseWindowState(),
                nowMillis,
                screenWidth,
                screenHeight
            ));
            commands.addAll(SpellVolumeHudPlanner.buildCommands(
                combatSnapshot.spellVolumeState(),
                screenWidth,
                screenHeight
            ));
            commands.addAll(EdgeFeedbackHudPlanner.buildCommands(
                combatSnapshot.combatHudState(),
                combatSnapshot.defenseWindowState(),
                combatSnapshot.castState(),
                nowMillis,
                screenWidth,
                screenHeight
            ));
            commands.addAll(StaminaBarHudPlanner.buildCommands(
                combatSnapshot.combatHudState(), screenWidth, screenHeight
            ));
            commands.addAll(ThroughputPeakHudPlanner.buildCommands(
                combatSnapshot.combatHudState(), screenWidth, screenHeight
            ));
            commands.addAll(StatusEffectHudPlanner.buildCommands(screenWidth, screenHeight));
            commands.addAll(DamageFloaterHudPlanner.buildCommands(screenWidth, screenHeight, nowMillis));
            commands.addAll(FlightHudPlanner.buildCommands(screenWidth, screenHeight, nowMillis));
            commands.addAll(TribulationBroadcastHudPlanner.buildCommands(screenWidth, screenHeight, nowMillis));
            commands.addAll(DerivedAttrIconHudPlanner.buildCommands(screenWidth, screenHeight));
            commands.addAll(NearDeathOverlayPlanner.buildCommands(
                combatSnapshot.combatHudState(), screenWidth, screenHeight
            ));
            // plan-alchemy-v1 §2.1 — 丹毒 mini bar(mellow/violent > 0 常驻, !ok 时红框警戒)
            // 暂时停用主 HUD 丹毒 mini bar,保留 planner 代码以便后续恢复。
            // commands.addAll(ContaminationHudPlanner.buildCommands(screenWidth, screenHeight));
        }
        if (BongClientFeatures.ENABLE_BOTANY_HUD) {
            commands.addAll(BotanyHudPlanner.buildCommands(
                widthMeasurer,
                screenWidth,
                screenHeight,
                botanyAnchor
            ));
        }
        commands.addAll(ExtractProgressHudPlanner.buildCommands(
            com.bong.client.tsy.ExtractStateStore.snapshot(),
            widthMeasurer,
            screenWidth,
            screenHeight,
            nowMillis
        ));
        commands.addAll(RealmCollapseHudPlanner.buildCommands(
            com.bong.client.state.RealmCollapseHudStateStore.snapshot(),
            widthMeasurer,
            screenWidth,
            screenHeight,
            nowMillis
        ));

        return List.copyOf(commands);
    }

    private static int normalizeWidth(int requestedWidth) {
        return requestedWidth > 0 ? requestedWidth : DEFAULT_TEXT_WIDTH;
    }
}
