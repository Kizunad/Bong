package com.bong.client.hud.svg;

import com.bong.client.combat.DefenseWindowStore;
import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.CombatHudStateStore;
import com.bong.client.combat.DerivedAttrFlags;
import com.bong.client.combat.CastOutcome;
import com.bong.client.combat.CastState;
import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.QuickSlotConfig;
import com.bong.client.combat.QuickSlotEntry;
import com.bong.client.combat.QuickUseSlotStore;
import com.bong.client.combat.SkillBarConfig;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.movement.MovementState;
import com.bong.client.movement.MovementStateStore;
import com.bong.client.combat.inspect.TechniquesListPanel;
import com.bong.client.movement.DashSkill;
import net.fabricmc.fabric.api.client.rendering.v1.HudRenderCallback;
import net.minecraft.client.MinecraftClient;

import java.util.List;

/** 仅由显式环境变量激活的 SVG 截图 fixture，不改变正常联机状态。 */
public final class SvgHudPreviewHarness {
    private static final String ENV_ENABLED = "BONG_SVG_HUD_PREVIEW";
    private static final String SCENARIO_PREFIX = "hud-";
    private static volatile Scenario scenario = Scenario.NONE;
    private static volatile long statusFixtureStartedAt;
    private static volatile long statusFixtureOffsetMs = -1;
    private static boolean loopStatusFixture;
    private static String movementShot = "";

    private SvgHudPreviewHarness() {
    }

    public static void install() {
        if (!"1".equals(System.getenv(ENV_ENABLED))) {
            return;
        }
        String liveScene = System.getenv("BONG_SVG_HUD_PREVIEW_SCENE");
        if (liveScene != null && !liveScene.isBlank()) {
            selectShot(liveScene);
            loopStatusFixture = scenario == Scenario.STATUS_EFFECTS;
        }
        // 由组合根先于生产 HUD 注册；同一渲染线程内不会被网络任务插入覆盖。
        HudRenderCallback.EVENT.register((context, tickDelta) -> apply(MinecraftClient.getInstance()));
    }

    /**
     * PreviewSession 在每张截图前选择一个 fixture。名称不是 HUD 场景时清空，避免
     * worldgen preview 沿用一帧前的本地状态。
     */
    public static void selectShot(String shotName) {
        if (!"1".equals(System.getenv(ENV_ENABLED))) {
            return;
        }
        scenario = Scenario.fromShotName(shotName);
        movementShot = shotName == null ? "" : shotName;
        statusFixtureStartedAt = System.currentTimeMillis();
        statusFixtureOffsetMs = switch (shotName == null ? "" : shotName) {
            case "hud-status-effects-entry" -> 200;
            case "hud-status-effects-travel" -> 2_450;
            case "hud-status-effects-settled" -> 4_000;
            case "hud-status-effects-warning" -> 8_000;
            case "hud-status-effects-exit" -> 12_140;
            default -> -1;
        };
        StatusEffectStore.clear();
        if (scenario == Scenario.NONE) {
            resetFixtures(System.currentTimeMillis());
        }
    }

    private static void installStatusEffectsFixture(long nowMs) {
            CombatHudStateStore.replaceAuthoritative(
                CombatHudState.createAuthoritative(1.0f, 1.0f, 1.0f, DerivedAttrFlags.none(), true)
            );
            long elapsed = statusFixtureOffsetMs >= 0 ? statusFixtureOffsetMs : Math.max(0, nowMs - statusFixtureStartedAt);
            if (loopStatusFixture) elapsed %= 30_000;
            long start = nowMs - elapsed;
            // 重建同一条演出时间线，避免真实服务端快照在两帧之间清空截图夹具。
            StatusEffectStore.clear();
            StatusEffectStore.replace(List.of(
                new StatusEffectStore.Effect("bleeding", "出血", StatusEffectStore.Kind.DOT,
                    3, 12_000L, 0xFFE04040, "预览", 2),
                new StatusEffectStore.Effect("stunned", "眩晕", StatusEffectStore.Kind.CONTROL,
                    1, 7_000L, 0xFFB060FF, "预览", 4),
                new StatusEffectStore.Effect("contaminationboost", "丹毒加重", StatusEffectStore.Kind.DEBUFF,
                    1, 25_000L, 0xFFBBC774, "预览", 3),
                new StatusEffectStore.Effect("speedboost", "疾行", StatusEffectStore.Kind.BUFF,
                    1, 28_000L, 0xFF60D060, "预览", 1)
            ), start);
            for (long age = 0; age <= Math.min(elapsed, 30_000); age += 50) {
                StatusEffectStore.presentation(start + age, StatusEffectStore.TOP_BAR_LIMIT);
            }
    }

    private static void apply(MinecraftClient client) {
        Scenario current = scenario;
        if (current == Scenario.NONE) {
            return;
        }
        long nowMs = System.currentTimeMillis();
        resetFixtures(nowMs, current != Scenario.STATUS_EFFECTS);
        client.inGameHud.getChatHud().clear(false);

        switch (current) {
            case JIEMAI -> DefenseWindowStore.open(60_000, nowMs);
            case STATUS_EFFECTS -> installStatusEffectsFixture(nowMs);
            case MOVEMENT -> installMovementFixture(nowMs);
            case QUICKBAR, CAST_GATHER, CAST_FORM, CAST_COMPLETE, CAST_INTERRUPTED ->
                installQuickbarFixture(current, nowMs);
            case NONE -> {
            }
        }
    }

    private static void installMovementFixture(long nowMs) {
        installQuickbarFixture(Scenario.QUICKBAR, nowMs);
        if (movementShot.equals("hud-movement-locked")) return;
        TechniquesListPanel.replace(List.of(new TechniquesListPanel.Technique(
            DashSkill.ID, "闪避", TechniquesListPanel.Grade.MORTAL, 0, true, "", "",
            "Awaken", List.of(), 0, 0, 40, 2.8f)));
        boolean cooling = movementShot.equals("hud-movement-cooldown");
        boolean dashing = movementShot.equals("hud-movement");
        boolean rejected = movementShot.equals("hud-movement-rejected");
        long activity = nowMs - (dashing || rejected ? 100 : 2_000);
        MovementState fixture = new MovementState(
            1, dashing, dashing ? MovementState.Action.DASHING : MovementState.Action.NONE,
            MovementState.ZoneKind.NORMAL, cooling ? 20 : dashing ? 38 : 0,
            40, 1.8, 36, 60, false, 1L, rejected ? "dash" : "", 0, 0, 0
        );
        MovementStateStore.replace(fixture, activity);
        MovementStateStore.replace(fixture, nowMs);
    }

    private static void installQuickbarFixture(Scenario current, long nowMs) {
        CombatHudStateStore.replaceAuthoritative(
            CombatHudState.createAuthoritative(.92f, .64f, .82f, DerivedAttrFlags.none(), true)
        );
        QuickUseSlotStore.replaceLocal(QuickSlotConfig.empty()
            .withSlot(0, new QuickSlotEntry("tie_bi_san", "铁壁散", 4000, 5000, ""))
            .withSlot(1, new QuickSlotEntry("leg_splint", "夹板", 4000, 5000, ""))
            .withCooldownUntil(1, nowMs + 5000));
        SkillBarStore.replace(SkillBarConfig.of(new SkillBarEntry[]{
            SkillBarEntry.item("stone_pickaxe", "石镐", 0, 0, ""),
            SkillBarEntry.skill("zhenmai_harden", "硬化", 4000, 5000,
                "bong-client:textures/gui/skill/zhenmai_harden.png")
        }, new long[]{0, 0}));
        SkillBarStore.setSelectedSlot(0);
        if (current == Scenario.QUICKBAR) {
            return;
        }
        // 使用本地预测入口，不能让截图状态成为服务端 accepted 的凭据。
        if (current == Scenario.CAST_GATHER) {
            CastStateStore.beginCast(0, 4000, nowMs - 1000);
        } else {
            CastStateStore.beginSkillBarCast(1, 4000, nowMs - 3000);
        }
        if (current == Scenario.CAST_COMPLETE) {
            CastStateStore.complete(nowMs);
        } else if (current == Scenario.CAST_INTERRUPTED) {
            CastStateStore.interrupt(CastOutcome.USER_CANCEL, nowMs);
        }
    }

    /** 将本 preview fixture 覆盖过的值恢复为空快照，不触发断线生命周期。 */
    private static void resetFixtures(long nowMs) {
        resetFixtures(nowMs, true);
    }

    private static void resetFixtures(long nowMs, boolean resetStatus) {
        CombatHudStateStore.clear();
        DefenseWindowStore.replaceSnapshot(null);
        if (resetStatus) StatusEffectStore.clear();
        MovementStateStore.replace(MovementState.empty(), nowMs);
        TechniquesListPanel.replace(List.of());
        CastStateStore.replacePrediction(CastState.idle());
        QuickUseSlotStore.replaceLocal(QuickSlotConfig.empty());
        SkillBarStore.replace(SkillBarConfig.empty());
        SkillBarStore.clearSelectedSlot();
    }

    private enum Scenario {
        NONE,
        JIEMAI,
        STATUS_EFFECTS,
        MOVEMENT,
        QUICKBAR,
        CAST_GATHER,
        CAST_FORM,
        CAST_COMPLETE,
        CAST_INTERRUPTED;

        static Scenario fromShotName(String shotName) {
            String name = shotName == null ? "" : shotName.trim().toLowerCase(java.util.Locale.ROOT);
            if (!name.startsWith(SCENARIO_PREFIX)) {
                return NONE;
            }
            return switch (name.substring(SCENARIO_PREFIX.length())) {
                case "jiemai" -> JIEMAI;
                case "status-effects", "status-effects-entry", "status-effects-travel", "status-effects-settled",
                    "status-effects-warning", "status-effects-exit" -> STATUS_EFFECTS;
                case "movement", "movement-ready", "movement-cooldown", "movement-rejected", "movement-locked" -> MOVEMENT;
                case "quickbar" -> QUICKBAR;
                case "cast-gather" -> CAST_GATHER;
                case "cast-form" -> CAST_FORM;
                case "cast-complete" -> CAST_COMPLETE;
                case "cast-interrupted" -> CAST_INTERRUPTED;
                default -> NONE;
            };
        }
    }
}
