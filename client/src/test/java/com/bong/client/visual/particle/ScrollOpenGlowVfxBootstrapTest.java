package com.bong.client.visual.particle;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-scroll-reading-v1 P2 — 锁死卷轴展开微光 {@code bong:scroll_open_glow} 已注册进
 * {@link VfxBootstrap#registerDefaults()}。
 *
 * <p>server 在 {@code ScrollOpen} 同帧 emit {@code spawn_particle}（event_id
 * {@code bong:scroll_open_glow}）；client 漏注册 → 粒子静默丢失（孤岛，同
 * {@code PackOperationVfxBootstrapTest} 记录的先例坑）。本测试在编译期外兜底。
 */
class ScrollOpenGlowVfxBootstrapTest {
    @Test
    void bootstrapRegistersScrollOpenGlow() {
        VfxRegistry.instance().clearForTests();

        VfxBootstrap.registerDefaults();

        assertTrue(
            VfxRegistry.instance().contains(ScrollOpenGlowPlayer.EVENT_ID),
            "scroll_open_glow event_id 必须注册，否则 server emit 的 bong:scroll_open_glow 微光静默丢失"
        );
    }

    /** event_id 字面值必须与 server emit 常量逐字符一致。 */
    @Test
    void eventIdMatchesServerEmitConstant() {
        assertEquals("bong:scroll_open_glow", ScrollOpenGlowPlayer.EVENT_ID.toString());
    }
}
