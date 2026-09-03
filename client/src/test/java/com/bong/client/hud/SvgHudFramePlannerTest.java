package com.bong.client.hud;

import com.bong.client.state.PlayerStateViewModel;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** 锁住 QI_RADAR 的应用层语义输出，避免表现后端重新读取业务状态。 */
class SvgHudFramePlannerTest {
    private static final int SCREEN_WIDTH = 320;
    private static final int SCREEN_HEIGHT = 180;
    private static final int EXPECTED_X = 96;
    private static final int EXPECTED_Y = 112;

    @Test
    void hidesRadarForAwakenAndInduceRealms() {
        assertFalse(SvgHudFramePlanner.plan(player("醒灵", 0.0), SCREEN_WIDTH, SCREEN_HEIGHT).visible(),
            "醒灵阶段尚未开放雷达，frame 必须隐藏");
        assertFalse(SvgHudFramePlanner.plan(player("引气", 0.0), SCREEN_WIDTH, SCREEN_HEIGHT).visible(),
            "引气阶段尚未开放雷达，frame 必须隐藏");
    }

    @Test
    void showsRadarFromCondenseWithStableBottomLeftLayout() {
        SvgHudFrame frame = SvgHudFramePlanner.plan(player("凝脉", 0.0), SCREEN_WIDTH, SCREEN_HEIGHT);

        assertTrue(frame.visible(), "凝脉阶段必须生成可提交的雷达 frame");
        assertEquals(EXPECTED_X, frame.x(), "雷达 x 坐标必须与既有 mini-body anchor 对齐");
        assertEquals(EXPECTED_Y, frame.y(), "雷达 y 坐标必须保持底部安全边距");
        assertEquals(1.0f, frame.scale(), "首个 SVG layer 使用固定逻辑缩放");
        assertEquals(0xFFFFFFFF, frame.tint(), "正常灵压必须使用默认白色 tint");
    }

    @Test
    void negativePressureUsesPurpleTint() {
        SvgHudFrame frame = SvgHudFramePlanner.plan(player("固元", -0.8), SCREEN_WIDTH, SCREEN_HEIGHT);

        assertTrue(frame.visible(), "负灵压不应关闭雷达，只改变视觉 tint");
        assertEquals(0xFFB58CE8, frame.tint(), "负灵压必须使用约定的紫色 tint");
    }

    @Test
    void nullPlayerAndInvalidResolutionFailClosed() {
        assertFalse(SvgHudFramePlanner.plan(null, SCREEN_WIDTH, SCREEN_HEIGHT).visible(),
            "缺少 player snapshot 时必须隐藏 SVG layer");
        assertFalse(SvgHudFramePlanner.plan(player("凝脉", 0.0), 0, SCREEN_HEIGHT).visible(),
            "零宽度 viewport 必须隐藏 SVG layer");
        assertFalse(SvgHudFramePlanner.plan(player("凝脉", 0.0), SCREEN_WIDTH, -1).visible(),
            "负高度 viewport 必须隐藏 SVG layer");
    }

    private static PlayerStateViewModel player(String realm, double localNegPressure) {
        return PlayerStateViewModel.create(
            realm,
            "offline:test",
            80.0,
            100.0,
            0.0,
            0.5,
            PlayerStateViewModel.PowerBreakdown.empty(),
            PlayerStateViewModel.SocialSnapshot.empty(),
            "jade",
            "青谷",
            0.5,
            localNegPressure
        );
    }
}
