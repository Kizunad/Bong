package com.bong.client.combat.inspect;

/**
 * 功法拖拽松手 → 动作 的纯决策。
 *
 * <p>抽成无 MC 依赖的纯函数，把「落点 × 来源」四种组合的状态转换锁死（见配套饱和测试）。
 * InspectScreen 的鼠标链只负责采集 {@code targetSlot}（{@code hotbarSlotAtScreen} 结果）与
 * {@code sourceSlot}（起手来源），动作分派全走这里。</p>
 */
public final class TechniqueDragDecision {
    private TechniqueDragDecision() {
    }

    public enum Action {
        /** 绑定到落点槽（从功法列表拖来，或拖回同一槽）。 */
        BIND,
        /** 移动绑定：从已绑槽 A 拖到不同的槽 B —— 绑 B 并清空 A。 */
        MOVE,
        /** 解绑消失：从已绑槽拖出、落在任何 1-9 槽之外（背包/空白等）。 */
        UNBIND,
        /** 取消：从功法列表拖出、落空 —— 不改绑定，保留选中（两步点击兜底）。 */
        CANCEL
    }

    /**
     * @param targetSlot 松手落点槽位；{@code < 0} 表示没落在任何 1-9 槽
     * @param sourceSlot 起手来源槽位；{@code >= 0} 表示从已绑槽拖出，{@code < 0} 表示从功法列表拖出
     */
    public static Action decide(int targetSlot, int sourceSlot) {
        boolean onSlot = targetSlot >= 0;
        boolean fromSlot = sourceSlot >= 0;
        if (onSlot) {
            return (fromSlot && targetSlot != sourceSlot) ? Action.MOVE : Action.BIND;
        }
        return fromSlot ? Action.UNBIND : Action.CANCEL;
    }
}
