package com.bong.client.craft;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-workbench-recipes-v1 P3.3 -- Pin 测试：锁住 SFX/VFX 常量值。
 *
 * <p>任何对音效 ID 或 VFX 参数的改动都会让对应 case 撞红，
 * 需要同步更新常量和测试。</p>
 */
class WorkbenchConstantsTest {

    // ---- SFX ID pin 测试 (7 条) ----

    @Test
    void sfxPlaceId() {
        assertEquals("bong:block.workbench.place", WorkbenchConstants.SFX_PLACE,
            "放置制作台音效 ID 应为 bong:block.workbench.place");
    }

    @Test
    void sfxBreakId() {
        assertEquals("bong:block.workbench.break", WorkbenchConstants.SFX_BREAK,
            "拆除制作台音效 ID 应为 bong:block.workbench.break");
    }

    @Test
    void sfxOpenId() {
        assertEquals("bong:block.workbench.open", WorkbenchConstants.SFX_OPEN,
            "打开制作台音效 ID 应为 bong:block.workbench.open");
    }

    @Test
    void sfxCraftStartId() {
        assertEquals("bong:craft.workbench.start", WorkbenchConstants.SFX_CRAFT_START,
            "开始制作音效 ID 应为 bong:craft.workbench.start");
    }

    @Test
    void sfxCraftTickId() {
        assertEquals("bong:craft.workbench.tick", WorkbenchConstants.SFX_CRAFT_TICK,
            "制作进行中音效 ID 应为 bong:craft.workbench.tick");
    }

    @Test
    void sfxCraftDoneId() {
        assertEquals("bong:craft.workbench.done", WorkbenchConstants.SFX_CRAFT_DONE,
            "制作完成音效 ID 应为 bong:craft.workbench.done");
    }

    @Test
    void sfxCraftFailId() {
        assertEquals("bong:craft.workbench.fail", WorkbenchConstants.SFX_CRAFT_FAIL,
            "制作失败音效 ID 应为 bong:craft.workbench.fail");
    }

    // ---- VFX color pin 测试 ----

    @Test
    void vfxCinnabarColorMatchesSpec() {
        assertEquals(0x8B3A3A, WorkbenchConstants.VFX_CINNABAR_COLOR,
            "丹砂红颜色应为 #8B3A3A (plan P3.3 spec)");
    }

    @Test
    void vfxQiBeamColorMatchesSpec() {
        assertEquals(0xA8C4E0, WorkbenchConstants.VFX_QI_BEAM_COLOR,
            "灵气丝蓝白颜色应为 #A8C4E0 (plan P3.3 spec)");
    }

    // ---- VFX parameter pin 测试 ----

    @Test
    void vfxIdleBurstInterval() {
        assertEquals(60, WorkbenchConstants.VFX_IDLE_BURST_INTERVAL,
            "空闲粒子 burst 间隔应为 60 tick");
    }

    @Test
    void vfxActiveBurstInterval() {
        assertEquals(10, WorkbenchConstants.VFX_ACTIVE_BURST_INTERVAL,
            "制作中粒子 burst 间隔应为 10 tick");
    }

    @Test
    void vfxBurstCountProgression() {
        assertTrue(WorkbenchConstants.VFX_IDLE_BURST_COUNT < WorkbenchConstants.VFX_ACTIVE_BURST_COUNT,
            "制作中 burst 粒子数(" + WorkbenchConstants.VFX_ACTIVE_BURST_COUNT
                + ")应大于空闲(" + WorkbenchConstants.VFX_IDLE_BURST_COUNT + ")");
        assertEquals(3, WorkbenchConstants.VFX_IDLE_BURST_COUNT);
        assertEquals(8, WorkbenchConstants.VFX_ACTIVE_BURST_COUNT);
    }

    @Test
    void vfxParticleLifetimeProgression() {
        assertTrue(WorkbenchConstants.VFX_IDLE_PARTICLE_LIFETIME
                <= WorkbenchConstants.VFX_ACTIVE_PARTICLE_LIFETIME,
            "制作中粒子寿命应 >= 空闲粒子寿命");
        assertEquals(10, WorkbenchConstants.VFX_IDLE_PARTICLE_LIFETIME);
        assertEquals(15, WorkbenchConstants.VFX_ACTIVE_PARTICLE_LIFETIME);
    }

    @Test
    void vfxCompleteBurst() {
        assertEquals(12, WorkbenchConstants.VFX_COMPLETE_BURST_COUNT,
            "完成 burst 粒子数应为 12");
        assertEquals(20, WorkbenchConstants.VFX_COMPLETE_PARTICLE_LIFETIME,
            "完成 burst 粒子寿命应为 20 tick");
    }

    @Test
    void vfxCompleteIconRise() {
        assertEquals(0.5f, WorkbenchConstants.VFX_COMPLETE_ICON_RISE, 1e-5f,
            "产出物品上浮高度应为 0.5 block");
    }

    @Test
    void vfxQiBeamParams() {
        assertEquals(1, WorkbenchConstants.VFX_QI_BEAM_WIDTH,
            "灵气丝宽度应为 1 px");
        assertEquals(30, WorkbenchConstants.VFX_QI_BEAM_LIFETIME,
            "灵气丝寿命应为 30 tick");
    }

    // ---- SFX ID 格式验证 ----

    @Test
    void allSfxIdsFollowBongNamespaceConvention() {
        String[] ids = {
            WorkbenchConstants.SFX_PLACE,
            WorkbenchConstants.SFX_BREAK,
            WorkbenchConstants.SFX_OPEN,
            WorkbenchConstants.SFX_CRAFT_START,
            WorkbenchConstants.SFX_CRAFT_TICK,
            WorkbenchConstants.SFX_CRAFT_DONE,
            WorkbenchConstants.SFX_CRAFT_FAIL,
        };
        for (String id : ids) {
            assertTrue(id.startsWith("bong:"),
                "SFX ID \"" + id + "\" 应以 bong: 开头 (namespace 约束)");
            assertTrue(id.contains("workbench"),
                "SFX ID \"" + id + "\" 应包含 workbench (制作台命名约束)");
        }
    }

    @Test
    void sfxIdCountIsSeven() {
        // Pin: exactly 7 SFX IDs as per plan P3.3 spec
        String[] ids = {
            WorkbenchConstants.SFX_PLACE,
            WorkbenchConstants.SFX_BREAK,
            WorkbenchConstants.SFX_OPEN,
            WorkbenchConstants.SFX_CRAFT_START,
            WorkbenchConstants.SFX_CRAFT_TICK,
            WorkbenchConstants.SFX_CRAFT_DONE,
            WorkbenchConstants.SFX_CRAFT_FAIL,
        };
        assertEquals(7, ids.length,
            "plan P3.3 规定 7 个音效 ID，实际 " + ids.length);
    }
}
