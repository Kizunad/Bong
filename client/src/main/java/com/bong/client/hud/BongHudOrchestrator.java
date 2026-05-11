package com.bong.client.hud;

import com.bong.client.BongClientFeatures;
import com.bong.client.combat.store.FalseSkinHudStateStore;
import com.bong.client.combat.store.TribulationStateStore;
import com.bong.client.combat.store.VortexStateStore;
import com.bong.client.identity.IdentityHudCornerLabel;
import com.bong.client.npc.NpcInteractionLogHudPlanner;
import com.bong.client.npc.NpcInteractionLogStore;
import com.bong.client.state.PlayerStateStore;
import com.bong.client.state.PlayerStateViewModel;
import com.bong.client.tsy.ExtractState;
import com.bong.client.tsy.ExtractStateStore;
import com.bong.client.ui.ClientConnectionStatusStore;
import com.bong.client.ui.ConnectionStatusIndicator;
import com.bong.client.visual.realm_vision.PerceptionEdgeState;
import com.bong.client.visual.realm_vision.PerceptionEdgeStateStore;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

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
        return buildCommands(
            snapshot,
            combat,
            nowMillis,
            widthMeasurer,
            maxTextWidth,
            screenWidth,
            screenHeight,
            botanyAnchor,
            HudRuntimeContext.empty()
        );
    }

    public static List<HudRenderCommand> buildCommands(
        BongHudStateSnapshot snapshot,
        CombatHudSnapshot combat,
        long nowMillis,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxTextWidth,
        int screenWidth,
        int screenHeight,
        BotanyProjection.Anchor botanyAnchor,
        HudRuntimeContext runtimeContext
    ) {
        BongHudStateSnapshot safeSnapshot = snapshot == null ? BongHudStateSnapshot.empty() : snapshot;
        CombatHudSnapshot combatSnapshot = combat == null ? CombatHudSnapshot.empty() : combat;
        HudRuntimeContext runtime = runtimeContext == null ? HudRuntimeContext.empty() : runtimeContext;
        HudImmersionMode.Mode mode = HudImmersionMode.resolve(
            combatSnapshot.combatHudState(),
            safeSnapshot.visualEffectState(),
            nowMillis
        );
        PlayerStateViewModel playerState = PlayerStateStore.snapshot();
        PerceptionEdgeState perceptionState = PerceptionEdgeStateStore.snapshot();
        ExtractState extractState = ExtractStateStore.snapshot();
        HudEnvironmentVariant environmentVariant = HudEnvironmentVariant.from(safeSnapshot.zoneState(), extractState);
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
        if (appendLocalNegPressure(
            commands,
            PlayerStateStore.snapshot(),
            widthMeasurer,
            normalizedWidth,
            BASELINE_X,
            nextY
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
            com.bong.client.atmosphere.ZoneAtmosphereHudPlanner.append(
                commands,
                com.bong.client.atmosphere.ZoneAtmosphereRenderer.currentCommand()
            );
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
            int seasonTint = screenWidth > 0 && screenHeight > 0
                ? com.bong.client.visual.season.SeasonVisuals.skyTintArgb(
                    com.bong.client.state.SeasonStateStore.snapshot(),
                    nowMillis
                )
                : 0;
            if (seasonTint != 0) {
                commands.add(HudRenderCommand.screenTint(HudRenderLayer.VISUAL, seasonTint));
            }
            commands.addAll(com.bong.client.season.SeasonBreakthroughOverlayHud.buildCommands(nowMillis));
            commands.addAll(com.bong.client.season.SeasonHintHudPlanner.buildCommands(
                com.bong.client.state.SeasonStateStore.snapshot(),
                screenWidth,
                screenHeight
            ));
            commands.addAll(OmenHudPlanner.buildCommands(
                com.bong.client.omen.OmenStateStore.snapshot(nowMillis),
                nowMillis,
                screenWidth,
                screenHeight
            ));
            commands.addAll(com.bong.client.visual.TsyPressureOverlay.buildCommands(
                playerState.localNegPressure(),
                screenWidth,
                screenHeight
            ));
        }

        commands.addAll(HudEnvironmentVariantPlanner.buildCommands(
            environmentVariant,
            safeSnapshot.zoneState(),
            extractState,
            screenWidth,
            screenHeight,
            nowMillis
        ));
        commands.addAll(QiDensityRadarHudPlanner.buildCommands(
            playerState,
            safeSnapshot.zoneState(),
            perceptionState,
            mode,
            environmentVariant,
            runtime,
            nowMillis,
            screenWidth,
            screenHeight
        ));
        if (HudRealmGate.atLeastCondense(playerState.realm())) {
            commands.addAll(DirectionalCompassHudPlanner.buildCommands(
                safeSnapshot.zoneState(),
                extractState,
                mode,
                runtime,
                widthMeasurer,
                screenWidth,
                screenHeight,
                nowMillis
            ));
        }
        commands.addAll(ThreatIndicatorHudPlanner.buildCommands(
            playerState,
            perceptionState,
            TribulationStateStore.snapshot(),
            runtime,
            nowMillis,
            screenWidth,
            screenHeight
        ));

        if (BongClientFeatures.ENABLE_COMBAT_HUD) {
            commands.addAll(MiniBodyHudPlanner.buildCommands(
                combatSnapshot.combatHudState(),
                combatSnapshot.physicalBody(),
                com.bong.client.inventory.state.InventoryStateStore.snapshot().equipped(),
                nowMillis,
                screenWidth,
                screenHeight,
                com.bong.client.state.SeasonStateStore.snapshot()
            ));
            if (combatSnapshot.combatHudState().active()) {
                commands.addAll(StyleBadgeHudPlanner.buildCommands(
                    combatSnapshot.unlockedStyles(),
                    screenWidth,
                    screenHeight
                ));
            }
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
            commands.addAll(CarrierHudPlanner.buildCommands(
                combatSnapshot.carrierState(),
                screenWidth,
                screenHeight
            ));
            commands.addAll(AnqiHudPlanner.buildCommands(
                AnqiHudStateStore.snapshot(),
                nowMillis,
                screenWidth,
                screenHeight
            ));
            commands.addAll(WoliuV2HudPlanner.buildCommands(
                VortexStateStore.snapshot(),
                screenWidth,
                screenHeight,
                nowMillis
            ));
            commands.addAll(DuguV2HudPlanner.buildCommands(
                DuguV2HudStateStore.snapshot(),
                screenWidth,
                screenHeight,
                nowMillis
            ));
            FalseSkinHudStateStore.State falseSkinSnapshot = FalseSkinHudStateStore.snapshot();
            commands.addAll(FalseSkinStackHud.buildCommands(
                falseSkinSnapshot,
                screenWidth,
                screenHeight
            ));
            commands.addAll(ContamLoadHud.buildCommands(
                falseSkinSnapshot,
                screenWidth,
                screenHeight
            ));
            commands.addAll(PoisonTraitHudPlanner.buildCommands(
                PoisonTraitHudStateStore.snapshot(),
                screenWidth,
                screenHeight,
                nowMillis
            ));
            commands.addAll(ChargingProgressBarHud.buildCommands(screenWidth, screenHeight));
            commands.addAll(ExhaustedGreyOverlay.buildCommands(screenWidth, screenHeight, nowMillis));
            commands.addAll(YidaoHudPlanner.buildCommands(
                com.bong.client.yidao.YidaoHudStateStore.snapshot(),
                widthMeasurer,
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
            commands.addAll(MovementHudPlanner.buildCommands(screenWidth, screenHeight, nowMillis));
            commands.addAll(ThroughputPeakHudPlanner.buildCommands(
                combatSnapshot.combatHudState(), screenWidth, screenHeight
            ));
            commands.addAll(StatusEffectHudPlanner.buildCommands(screenWidth, screenHeight));
            commands.addAll(DamageFloaterHudPlanner.buildCommands(screenWidth, screenHeight, nowMillis));
            commands.addAll(FlightHudPlanner.buildCommands(screenWidth, screenHeight, nowMillis));
            commands.addAll(TribulationBroadcastHudPlanner.buildCommands(screenWidth, screenHeight, nowMillis));
        commands.addAll(TargetInfoHudPlanner.buildCommands(
            TargetInfoStateStore.snapshot(),
            nowMillis,
            widthMeasurer,
            screenWidth,
            screenHeight
        ));
        commands.addAll(com.bong.client.tsy.TsyBossHealthBar.buildCommands(
            com.bong.client.tsy.TsyBossHealthStore.snapshot(),
            nowMillis,
            widthMeasurer,
            screenWidth
        ));
        commands.addAll(com.bong.client.tsy.TsyCorpseDeathVfx.buildCommands(
            com.bong.client.tsy.TsyDeathVfxStore.snapshot(),
            nowMillis,
            screenWidth,
            screenHeight
        ));
        commands.addAll(NpcInteractionLogHudPlanner.buildCommands(
            NpcInteractionLogStore.snapshot(),
            NpcInteractionLogStore.visible(),
            widthMeasurer,
            screenWidth,
            screenHeight
        ));
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
        commands.addAll(GatheringProgressHud.buildCommands(
            widthMeasurer,
            screenWidth,
            screenHeight,
            nowMillis
        ));
        commands.addAll(ForgeProgressHudPlanner.buildCommands(screenWidth, screenHeight, nowMillis));
        commands.addAll(AlchemyProgressHudPlanner.buildCommands(screenWidth, screenHeight));
        commands.addAll(CoffinHudPlanner.buildCommands(screenWidth, screenHeight));
        commands.addAll(LingtianOverlayHudPlanner.buildCommands(
            com.bong.client.lingtian.state.LingtianSessionStore.snapshot(),
            screenWidth,
            screenHeight,
            com.bong.client.state.SeasonStateStore.snapshot()
        ));
        commands.addAll(ExtractProgressHudPlanner.buildCommands(
            com.bong.client.tsy.ExtractStateStore.snapshot(),
            widthMeasurer,
            screenWidth,
            screenHeight,
            nowMillis
        ));
        commands.addAll(SearchProgressHudPlanner.buildCommands(
            SearchHudStateStore.snapshot(),
            screenWidth,
            screenHeight
        ));
        commands.addAll(RealmCollapseHudPlanner.buildCommands(
            com.bong.client.state.RealmCollapseHudStateStore.snapshot(),
            widthMeasurer,
            screenWidth,
            screenHeight,
            nowMillis
        ));
        commands.addAll(MeridianOpenHudPlanner.buildCommands(widthMeasurer, screenWidth, screenHeight));
        commands.addAll(IdentityHudCornerLabel.buildCommands(widthMeasurer, screenWidth));
        commands.addAll(ConnectionStatusIndicator.buildCommands(
            ClientConnectionStatusStore.snapshot(nowMillis),
            screenWidth,
            screenHeight
        ));

        List<HudRenderCommand> layoutCommands = HudLayoutPreset.filter(
            commands,
            mode,
            HudLayoutPreferenceStore.density(),
            nowMillis
        );
        return List.copyOf(HudImmersionMode.applyImmersiveAlpha(
            layoutCommands,
            mode,
            safeSnapshot.visualEffectState(),
            runtime,
            nowMillis
        ));
    }

    private static int normalizeWidth(int requestedWidth) {
        return requestedWidth > 0 ? requestedWidth : DEFAULT_TEXT_WIDTH;
    }

    static boolean appendLocalNegPressure(
        List<HudRenderCommand> commands,
        PlayerStateViewModel playerState,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int maxWidth,
        int x,
        int y
    ) {
        PlayerStateViewModel safePlayerState = playerState == null ? PlayerStateViewModel.empty() : playerState;
        if (safePlayerState.localNegPressure() >= 0.0 || widthMeasurer == null || maxWidth <= 0) {
            return false;
        }

        String text = String.format(Locale.ROOT, "灵压 %.2f", safePlayerState.localNegPressure());
        String clipped = HudTextHelper.clipToWidth(text, maxWidth, widthMeasurer);
        if (clipped.isEmpty()) {
            return false;
        }
        commands.add(HudRenderCommand.text(HudRenderLayer.ZONE, clipped, x, y, 0x9FD3FF));
        return true;
    }
}
