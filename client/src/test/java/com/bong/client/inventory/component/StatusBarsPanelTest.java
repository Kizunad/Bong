package com.bong.client.inventory.component;

import com.bong.client.PlayerStateState;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

/**
 * 背包状态条的数据源解析：境界/真元必须优先 player_state 活通道，
 * inventory_snapshot 冻结值只做兜底。回归钉子——曾经只读快照值导致
 * 开包期间 /realm set、境界突破、真元涨落全部不刷新（境界卡在开包瞬间的旧值）。
 */
class StatusBarsPanelTest {

    private static PlayerStateState.PlayerStateSnapshot live(String realm, double qi, double qiMax) {
        return new PlayerStateState.PlayerStateSnapshot(realm, qi, qiMax, 0.0, 0.5, "spawn", 0L);
    }

    @Test
    void resolveRealmPrefersLiveSnapshot() {
        assertEquals(
            "Void",
            StatusBarsPanel.resolveRealm("Awaken", live("Void", 10, 100)),
            "有活的 player_state 数据时必须显示活通道境界（Void），"
                + "而不是 inventory_snapshot 里开包瞬间冻结的 Awaken——否则 /realm set 后背包 UI 卡旧境界"
        );
    }

    @Test
    void resolveRealmFallsBackToSnapshotWhenNoLiveData() {
        assertEquals(
            "Condense",
            StatusBarsPanel.resolveRealm("Condense", null),
            "尚未收到任何 player_state payload（如刚连接）时应回退快照值而不是显示空白"
        );
        assertEquals(
            "Condense",
            StatusBarsPanel.resolveRealm("Condense", live("", 10, 100)),
            "活通道 realm 为空串（异常 payload）时应回退快照值"
        );
    }

    @Test
    void resolveQiRatioPrefersLiveSnapshot() {
        assertEquals(
            0.25,
            StatusBarsPanel.resolveQiRatio(0.9, live("Void", 25, 100)),
            1e-9,
            "有活数据时真元比例必须按活通道 25/100=0.25 算，而不是快照的 0.9"
        );
    }

    @Test
    void resolveQiRatioClampsToUnitRange() {
        assertEquals(
            1.0,
            StatusBarsPanel.resolveQiRatio(0.5, live("Void", 250, 100)),
            1e-9,
            "qi 超上限（250/100）必须钳到 1.0，不能画出溢出的条"
        );
    }

    @Test
    void resolveQiRatioFallsBackWhenNoLiveDataOrInvalidMax() {
        assertEquals(
            0.7,
            StatusBarsPanel.resolveQiRatio(0.7, null),
            1e-9,
            "无活数据时回退快照比例"
        );
        assertEquals(
            0.7,
            StatusBarsPanel.resolveQiRatio(0.7, live("Void", 10, 0.0)),
            1e-9,
            "活数据 spiritQiMax<=0（除零风险）时必须回退快照比例"
        );
    }
}
