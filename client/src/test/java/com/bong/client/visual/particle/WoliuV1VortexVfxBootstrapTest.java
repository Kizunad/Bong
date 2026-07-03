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

    private static VortexSpiralPlayer.EffectSpec specFor(net.minecraft.util.Identifier eventId) {
        return VortexSpiralPlayer.effectSpec(
            new com.bong.client.network.VfxEventPayload.SpawnParticle(
                eventId,
                new double[] {0.0, 64.0, 0.0},
                java.util.Optional.empty(),
                java.util.OptionalInt.empty(),
                java.util.Optional.empty(),
                java.util.OptionalInt.empty(),
                java.util.OptionalInt.empty()
            )
        );
    }
}
