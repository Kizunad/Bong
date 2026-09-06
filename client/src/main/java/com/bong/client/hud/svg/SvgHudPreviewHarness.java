package com.bong.client.hud.svg;

import com.bong.client.combat.DefenseWindowStore;
import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.CombatHudStateStore;
import com.bong.client.combat.DerivedAttrFlags;
import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.movement.MovementState;
import com.bong.client.movement.MovementStateStore;
import net.fabricmc.fabric.api.client.rendering.v1.HudRenderCallback;
import net.minecraft.client.MinecraftClient;

import java.util.List;

/** 仅由显式环境变量激活的 SVG 截图 fixture，不改变正常联机状态。 */
public final class SvgHudPreviewHarness {
    private static final String ENV_ENABLED = "BONG_SVG_HUD_PREVIEW";
    private static final String SCENARIO_PREFIX = "hud-";
    private static volatile Scenario scenario = Scenario.NONE;

    private SvgHudPreviewHarness() {
    }

    public static void install() {
        if (!"1".equals(System.getenv(ENV_ENABLED))) {
            return;
        }
        // 预览开关只在显式 fixture 环境中打开，生产联机不会加载示例面板。
        SvgHudBackend.enablePreviewExample();
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
        if (scenario == Scenario.NONE) {
            resetFixtures(System.currentTimeMillis());
            SvgHudBackend.enablePreviewExample();
        } else {
            SvgHudBackend.disablePreviewExample();
        }
    }

    private static void installStatusEffectsFixture() {
            CombatHudStateStore.replaceAuthoritative(
                CombatHudState.createAuthoritative(1.0f, 1.0f, 1.0f, DerivedAttrFlags.none(), true)
            );
            StatusEffectStore.replace(List.of(
                new StatusEffectStore.Effect("bleeding", "出血", StatusEffectStore.Kind.DOT,
                    3, 22_000L, 0xFFE04040, "预览", 2),
                new StatusEffectStore.Effect("stunned", "眩晕", StatusEffectStore.Kind.CONTROL,
                    1, 14_000L, 0xFFB060FF, "预览", 4),
                new StatusEffectStore.Effect("speedboost", "疾行", StatusEffectStore.Kind.BUFF,
                    1, 28_000L, 0xFF60D060, "预览", 1)
            ));
            StatusEffectStore.setCultivationAcceleration(1.6);
    }

    private static void apply(MinecraftClient client) {
        Scenario current = scenario;
        if (current == Scenario.NONE) {
            return;
        }
        long nowMs = System.currentTimeMillis();
        resetFixtures(nowMs);
        client.inGameHud.getChatHud().clear(false);

        switch (current) {
            case JIEMAI -> DefenseWindowStore.open(60_000, nowMs);
            case STATUS_EFFECTS -> installStatusEffectsFixture();
            case MOVEMENT -> MovementStateStore.replace(new MovementState(
                1.25, true, MovementState.Action.DASHING, MovementState.ZoneKind.NORMAL,
                18L, 1.8, 36.0, 60.0, false, 1L, "", 0L, 0L, 0L
            ), nowMs);
            case NONE -> {
            }
        }
    }

    /** 将本 preview fixture 覆盖过的值恢复为空快照，不触发断线生命周期。 */
    private static void resetFixtures(long nowMs) {
        CombatHudStateStore.clear();
        DefenseWindowStore.replaceSnapshot(null);
        StatusEffectStore.replace(List.of());
        StatusEffectStore.setCultivationAcceleration(1.0);
        MovementStateStore.replace(MovementState.empty(), nowMs);
    }

    private enum Scenario {
        NONE,
        JIEMAI,
        STATUS_EFFECTS,
        MOVEMENT;

        static Scenario fromShotName(String shotName) {
            String name = shotName == null ? "" : shotName.trim().toLowerCase(java.util.Locale.ROOT);
            if (!name.startsWith(SCENARIO_PREFIX)) {
                return NONE;
            }
            return switch (name.substring(SCENARIO_PREFIX.length())) {
                case "jiemai" -> JIEMAI;
                case "status-effects" -> STATUS_EFFECTS;
                case "movement" -> MOVEMENT;
                default -> NONE;
            };
        }
    }
}
