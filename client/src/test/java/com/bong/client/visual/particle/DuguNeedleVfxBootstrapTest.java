package com.bong.client.visual.particle;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 锁死蛊道两招粒子 event_id 与 server emit 端 {@code VFX_DUGU_NEEDLE_BOLT} /
 * {@code VFX_DUGU_POISON_INFUSE} 的逐字符对齐——server 端发 {@code bong:dugu_needle_bolt}
 * 而 client 注册成别的 path 会导致粒子静默丢失（孤岛），此测试在编译期外兜底。
 */
public class DuguNeedleVfxBootstrapTest {
    @Test
    void bootstrapRegistersBothDuguNeedleParticleRoutes() {
        VfxRegistry.instance().clearForTests();

        VfxBootstrap.registerDefaults();

        assertTrue(
            VfxRegistry.instance().contains(DuguNeedleVfxPlayer.DUGU_NEEDLE_BOLT),
            "凝针弹道 event_id 必须注册，否则 server emit 的 bong:dugu_needle_bolt 粒子静默丢失"
        );
        assertTrue(
            VfxRegistry.instance().contains(DuguNeedleVfxPlayer.DUGU_POISON_INFUSE),
            "灌毒蛊毒雾 event_id 必须注册，否则 server emit 的 bong:dugu_poison_infuse 粒子静默丢失"
        );
    }

    /** event_id 字面值必须与 server 常量逐字符一致（server: VFX_DUGU_NEEDLE_BOLT / VFX_DUGU_POISON_INFUSE）。 */
    @Test
    void eventIdsMatchServerEmitConstants() {
        assertEquals("bong:dugu_needle_bolt", DuguNeedleVfxPlayer.DUGU_NEEDLE_BOLT.toString());
        assertEquals("bong:dugu_poison_infuse", DuguNeedleVfxPlayer.DUGU_POISON_INFUSE.toString());
    }
}
