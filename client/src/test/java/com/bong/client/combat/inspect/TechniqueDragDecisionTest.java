package com.bong.client.combat.inspect;

import com.bong.client.combat.inspect.TechniqueDragDecision.Action;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

/**
 * {@link TechniqueDragDecision#decide} 的状态转换饱和测试。
 *
 * <p>两个轴：落点（落在某 1-9 槽 / 落在槽外）× 来源（从功法列表 / 从已绑槽）= 四象限，
 * 每象限锁一条专属动作；再加「拖回同一槽」与槽位边界(0 / 8 / -1)的 off-by-one。</p>
 */
class TechniqueDragDecisionTest {

    @Test
    void fromListOntoSlotBinds() {
        assertEquals(Action.BIND, TechniqueDragDecision.decide(3, -1),
            "从功法列表(源 -1)拖到槽 3 → 绑定");
    }

    @Test
    void fromSlotOntoDifferentSlotMoves() {
        assertEquals(Action.MOVE, TechniqueDragDecision.decide(3, 5),
            "从已绑槽 5 拖到不同的槽 3 → 移动绑定(绑 3 清 5)");
    }

    @Test
    void fromSlotBackOntoSameSlotBindsNotMoves() {
        // 拖起又放回原槽：必须是 BIND（不清源），否则会把自己清掉变成空槽。
        assertEquals(Action.BIND, TechniqueDragDecision.decide(4, 4),
            "从槽 4 拖回槽 4(同槽) → 应 BIND 而非 MOVE，避免误清源槽");
    }

    @Test
    void fromSlotDroppedOutsideUnbinds() {
        assertEquals(Action.UNBIND, TechniqueDragDecision.decide(-1, 5),
            "从已绑槽 5 拖出、落在所有 1-9 槽之外 → 解绑消失");
    }

    @Test
    void fromListDroppedOutsideCancels() {
        assertEquals(Action.CANCEL, TechniqueDragDecision.decide(-1, -1),
            "从功法列表拖出、落空 → 取消(不改绑定，保留选中)");
    }

    @Test
    void slotIndexBoundariesZeroAndEight() {
        // 槽位 0..8 均为合法落点；负为「无落点」。边界两端各验一次。
        assertEquals(Action.BIND, TechniqueDragDecision.decide(0, -1),
            "落点槽 0(首槽)合法 → BIND");
        assertEquals(Action.BIND, TechniqueDragDecision.decide(8, 8),
            "落点槽 8(末槽)且同源同槽 → BIND");
        assertEquals(Action.MOVE, TechniqueDragDecision.decide(8, 0),
            "源槽 0 → 落点槽 8 → MOVE");
        assertEquals(Action.UNBIND, TechniqueDragDecision.decide(-1, 0),
            "源槽 0(首槽)拖出落空 → UNBIND");
    }
}
