package com.bong.client.visual.particle;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 锁死绝灵涡流（woliu v1 {@code woliu.vortex} 长驻负灵域）三态粒子 event_id 与 server
 * {@code emit_woliu_v1_vortex_visual_triggers} 端常量（{@code VFX_WOLIU_V1_FIELD_OPEN} /
 * {@code _FIELD_AMBIENT} / {@code _BACKFIRE}）的逐字符对齐。v1 此前完全没有 AV emit，
 * 本簇是首次接通——注册缺一个该态粒子就静默丢失。
 */
public class WoliuV1VortexVfxBootstrapTest {
    @Test
    void bootstrapRegistersAllWoliuV1FieldRoutes() {
        VfxRegistry.instance().clearForTests();

        VfxBootstrap.registerDefaults();

        assertTrue(
            VfxRegistry.instance().contains(VortexSpiralPlayer.WOLIU_V1_FIELD_OPEN),
            "开涡 event_id 必须注册，否则 server emit 的 bong:woliu_vortex_field 粒子静默丢失"
        );
        assertTrue(
            VfxRegistry.instance().contains(VortexSpiralPlayer.WOLIU_V1_FIELD_AMBIENT),
            "存续涡环 event_id 必须注册，否则领域存续期间无任何可见反馈"
        );
        assertTrue(
            VfxRegistry.instance().contains(VortexSpiralPlayer.WOLIU_V1_BACKFIRE),
            "反噬爆裂 event_id 必须注册，否则断经反噬只有音效没有画面"
        );
    }

    /** event_id 字面值必须与 server emit 端常量逐字符一致。 */
    @Test
    void eventIdsMatchServerEmitConstants() {
        assertEquals("bong:woliu_vortex_field", VortexSpiralPlayer.WOLIU_V1_FIELD_OPEN.toString());
        assertEquals(
            "bong:woliu_vortex_field_ambient",
            VortexSpiralPlayer.WOLIU_V1_FIELD_AMBIENT.toString()
        );
        assertEquals(
            "bong:woliu_vortex_backfire",
            VortexSpiralPlayer.WOLIU_V1_BACKFIRE.toString()
        );
    }

    /** 三态 effectSpec 必须命中差异化路线（吞噬螺旋/环境涡/湍流爆），不落默认 SPIRAL。 */
    @Test
    void effectSpecRoutesAreDifferentiated() {
        assertEquals(
            VortexSpiralPlayer.Route.SWALLOWING_SPIRAL,
            specFor(VortexSpiralPlayer.WOLIU_V1_FIELD_OPEN).route(),
            "开涡应走吞噬螺旋路线（吸入感），落到默认 SPIRAL 说明 effectSpec 分支丢了"
        );
        assertEquals(
            VortexSpiralPlayer.Route.VORTEX_AMBIENT,
            specFor(VortexSpiralPlayer.WOLIU_V1_FIELD_AMBIENT).route(),
            "存续应走低频环境涡路线"
        );
        assertEquals(
            VortexSpiralPlayer.Route.TURBULENCE_BURST,
            specFor(VortexSpiralPlayer.WOLIU_V1_BACKFIRE).route(),
            "反噬应走湍流爆裂路线"
        );
    }

    /** 三态 effectSpec 对 count/durationTicks 的裁剪边界：越界输入必须收敛到 [min, max]。 */
    @Test
    void effectSpecClampsCountAndDurationBoundaries() {
        // 开涡：count ∈ [12, 56]，duration ∈ [15, 70]
        assertClampedSpec(VortexSpiralPlayer.WOLIU_V1_FIELD_OPEN, 12, 56, 15, 70);
        // 存续：count ∈ [6, 32]，duration ∈ [12, 60]
        assertClampedSpec(VortexSpiralPlayer.WOLIU_V1_FIELD_AMBIENT, 6, 32, 12, 60);
        // 反噬：count ∈ [16, 64]，duration ∈ [10, 44]
        assertClampedSpec(VortexSpiralPlayer.WOLIU_V1_BACKFIRE, 16, 64, 10, 44);
    }

    /** strength 走 clamp01：负值收敛到 0、超 1 收敛到 1（三态同口径）。 */
    @Test
    void effectSpecClampsStrengthToUnitInterval() {
        for (net.minecraft.util.Identifier eventId : java.util.List.of(
            VortexSpiralPlayer.WOLIU_V1_FIELD_OPEN,
            VortexSpiralPlayer.WOLIU_V1_FIELD_AMBIENT,
            VortexSpiralPlayer.WOLIU_V1_BACKFIRE
        )) {
            assertEquals(
                1.0,
                specFor(eventId, java.util.OptionalInt.empty(), java.util.OptionalInt.empty(),
                    java.util.Optional.of(9.0)).strength(),
                1e-9,
                eventId + " strength=9.0 应被 clamp01 收敛到 1.0"
            );
            assertEquals(
                0.0,
                specFor(eventId, java.util.OptionalInt.empty(), java.util.OptionalInt.empty(),
                    java.util.Optional.of(-3.0)).strength(),
                1e-9,
                eventId + " strength=-3.0 应被 clamp01 收敛到 0.0"
            );
        }
    }

    private static void assertClampedSpec(
        net.minecraft.util.Identifier eventId,
        int countMin, int countMax, int durationMin, int durationMax
    ) {
        // 下界外（min-1）→ 收敛到 min；上界外（max+1 与远超值）→ 收敛到 max；界内原样透传。
        VortexSpiralPlayer.EffectSpec below = specFor(eventId,
            java.util.OptionalInt.of(countMin - 1), java.util.OptionalInt.of(durationMin - 1),
            java.util.Optional.empty());
        assertEquals(countMin, below.count(),
            eventId + " count=" + (countMin - 1) + "（下界外 1）应收敛到 " + countMin);
        assertEquals(durationMin, below.maxAge(),
            eventId + " duration=" + (durationMin - 1) + "（下界外 1）应收敛到 " + durationMin);

        VortexSpiralPlayer.EffectSpec above = specFor(eventId,
            java.util.OptionalInt.of(countMax + 1), java.util.OptionalInt.of(durationMax + 1),
            java.util.Optional.empty());
        assertEquals(countMax, above.count(),
            eventId + " count=" + (countMax + 1) + "（上界外 1）应收敛到 " + countMax);
        assertEquals(durationMax, above.maxAge(),
            eventId + " duration=" + (durationMax + 1) + "（上界外 1）应收敛到 " + durationMax);

        VortexSpiralPlayer.EffectSpec inside = specFor(eventId,
            java.util.OptionalInt.of(countMin), java.util.OptionalInt.of(durationMax),
            java.util.Optional.empty());
        assertEquals(countMin, inside.count(), eventId + " 界内 count 应原样透传");
        assertEquals(durationMax, inside.maxAge(), eventId + " 界内 duration 应原样透传");
    }

    private static VortexSpiralPlayer.EffectSpec specFor(net.minecraft.util.Identifier eventId) {
        return specFor(eventId, java.util.OptionalInt.empty(), java.util.OptionalInt.empty(),
            java.util.Optional.empty());
    }

    private static VortexSpiralPlayer.EffectSpec specFor(
        net.minecraft.util.Identifier eventId,
        java.util.OptionalInt count,
        java.util.OptionalInt durationTicks,
        java.util.Optional<Double> strength
    ) {
        return VortexSpiralPlayer.effectSpec(
            new com.bong.client.network.VfxEventPayload.SpawnParticle(
                eventId,
                new double[] {0.0, 64.0, 0.0},
                java.util.Optional.empty(),
                java.util.OptionalInt.empty(),
                strength,
                count,
                durationTicks
            )
        );
    }
}
