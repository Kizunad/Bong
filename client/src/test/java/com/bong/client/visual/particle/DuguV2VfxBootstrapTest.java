package com.bong.client.visual.particle;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 锁死蛊道 v2 三张专属粒子 event_id 与 server {@code combat::dugu_v2::skills::visual_for()}
 * 各招 {@code particle_id} 的逐字符对齐——server {@code emit_dugu_v2_visual_triggers} 发
 * {@code bong:dugu_taint_pulse} 等而 client 未注册会导致 VfxRegistry 查表 miss、粒子静默
 * 丢弃（本簇贴图自 #173 起存在但从未接通，正是这种孤岛）。
 */
public class DuguV2VfxBootstrapTest {
    @Test
    void bootstrapRegistersAllDuguV2ParticleRoutes() {
        VfxRegistry.instance().clearForTests();

        VfxBootstrap.registerDefaults();

        for (net.minecraft.util.Identifier eventId : DuguV2VfxPlayer.EVENT_IDS) {
            assertTrue(
                VfxRegistry.instance().contains(eventId),
                "蛊道 v2 event_id " + eventId + " 必须注册，否则 server emit 的粒子静默丢失"
            );
        }
    }

    /** event_id 字面值必须与 server visual_for() 各招 particle_id 逐字符一致。 */
    @Test
    void eventIdsMatchServerVisualForParticleIds() {
        assertEquals("bong:dugu_taint_pulse", DuguV2VfxPlayer.DUGU_TAINT_PULSE.toString());
        assertEquals("bong:dugu_dark_green_mist", DuguV2VfxPlayer.DUGU_DARK_GREEN_MIST.toString());
        assertEquals("bong:dugu_reverse_burst", DuguV2VfxPlayer.DUGU_REVERSE_BURST.toString());
        assertEquals(3, DuguV2VfxPlayer.EVENT_IDS.size(), "蛊道 v2 专属粒子应恰好 3 个 event_id");
    }
}
