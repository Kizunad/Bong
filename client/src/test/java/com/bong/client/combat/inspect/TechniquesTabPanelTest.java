package com.bong.client.combat.inspect;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 锁定功法拖拽起手的命中几何 {@link TechniquesTabPanel#rowHit}。
 *
 * <p>约定为半开区间 {@code [x, x+w) × [y, y+h)}，与 hotbar 槽位命中（{@code sx >= x && sx < x+cs}）
 * 同一约定——相邻行/槽边界不能同时命中两个，否则拖拽起手会绑错行。off-by-one 全压在边界这几条上。</p>
 *
 * <p>owo 组件树与事件链需要 MC 运行环境，无法在 headless 单测里构造 TechniquesTabPanel；
 * 此处只锁纯静态命中逻辑（与 MovementKeybindingsTest 同策略），拖拽链路的端到端行为靠真机验证。</p>
 */
class TechniquesTabPanelTest {

    // 一个参考行：左上角 (20,40)，宽 170 高 20（功法行实际尺寸量级）。
    private static final int X = 20;
    private static final int Y = 40;
    private static final int W = 170;
    private static final int H = 20;

    @Test
    void centerPointHits() {
        assertTrue(TechniquesTabPanel.rowHit(X + W / 2.0, Y + H / 2.0, X, Y, W, H),
            "行正中应命中");
    }

    @Test
    void topLeftCornerIsInclusive() {
        assertTrue(TechniquesTabPanel.rowHit(X, Y, X, Y, W, H),
            "左上角 (x,y) 属于本行（区间左闭），应命中");
    }

    @Test
    void rightEdgeIsExclusive() {
        // px == x+w 落到「下一列」起点，半开区间右开 → 不命中本行。
        assertFalse(TechniquesTabPanel.rowHit(X + W, Y + H / 2.0, X, Y, W, H),
            "右边界 x+w 区间右开，不应命中本行（否则与右侧滚动条/相邻区域抢命中）");
    }

    @Test
    void bottomEdgeIsExclusive() {
        // py == y+h 落到「下一行」起点 → 归下一行，避免行间缝隙双命中。
        assertFalse(TechniquesTabPanel.rowHit(X + W / 2.0, Y + H, X, Y, W, H),
            "下边界 y+h 区间下开，应归下一行而非本行");
    }

    @Test
    void justInsideRightEdgeHits() {
        assertTrue(TechniquesTabPanel.rowHit(X + W - 0.001, Y + H - 0.001, X, Y, W, H),
            "恰好在右下边界内侧（x+w-ε, y+h-ε）仍属本行");
    }

    @Test
    void justOutsideLeftEdgeMisses() {
        assertFalse(TechniquesTabPanel.rowHit(X - 0.001, Y + H / 2.0, X, Y, W, H),
            "左边界外侧（x-ε）不命中");
        assertFalse(TechniquesTabPanel.rowHit(X + W / 2.0, Y - 0.001, X, Y, W, H),
            "上边界外侧（y-ε）不命中");
    }

    @Test
    void pointFarOutsideMisses() {
        assertFalse(TechniquesTabPanel.rowHit(0, 0, X, Y, W, H),
            "完全在行外（左上方）不命中");
        assertFalse(TechniquesTabPanel.rowHit(1000, 1000, X, Y, W, H),
            "完全在行外（右下方）不命中");
    }

    @Test
    void zeroSizeRowNeverHits() {
        // 列表为空 / 行尚未布局时宽高可能为 0，命中必须恒 false，避免拖到空行起手。
        assertFalse(TechniquesTabPanel.rowHit(X, Y, X, Y, 0, 0),
            "零尺寸行（半开区间退化为空集）任何点都不应命中");
        assertFalse(TechniquesTabPanel.rowHit(X, Y, X, Y, 0, H),
            "宽 0 的行不命中");
        assertFalse(TechniquesTabPanel.rowHit(X, Y, X, Y, W, 0),
            "高 0 的行不命中");
    }

    @Test
    void negativeOriginFromScrollOffsetStillWorks() {
        // 滚动后行可能被推到视口上方，x()/y() 变负；命中几何对负原点同样成立。
        assertTrue(TechniquesTabPanel.rowHit(-30, -10, -50, -20, W, H),
            "原点为负（向上滚动出视口的行）时几何仍正确，内部点命中");
        assertFalse(TechniquesTabPanel.rowHit(-50 + W, -20, -50, -20, W, H),
            "负原点下右边界仍区间右开，不命中");
    }
}
