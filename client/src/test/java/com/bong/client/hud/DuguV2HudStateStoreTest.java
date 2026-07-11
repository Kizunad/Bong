package com.bong.client.hud;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-bughunt-dugu-v2-hud-disconnect-bleed-v1 — 毒蛊 v2 HUD store 本体的隔离单测。
 *
 * <p>server 的 {@code dugu_v2_skill_cast} / {@code dugu_v2_self_cure} /
 * {@code dugu_v2_shroud_active} / {@code permanent_qi_max_decay_applied} bridge 只在事件
 * 发生时推增量，没有 join/disconnect reset payload；此前 store 只有
 * {@link DuguV2HudStateStore#resetForTests()}（测试专用），生产代码路径完全没有清理入口。
 * 本文件锁住 {@link DuguV2HudStateStore#clearOnDisconnect()} 自身的行为契约，
 * {@link com.bong.client.BongNetworkHandlerTest} 另锁住它被正确接入
 * {@code clearClientStateOnDisconnect()} 断线清理链。
 */
class DuguV2HudStateStoreTest {

    @AfterEach
    void resetStore() {
        DuguV2HudStateStore.resetForTests();
    }

    @Test
    void defaultSnapshotIsNone() {
        assertEquals(
            DuguV2HudStateStore.State.NONE,
            DuguV2HudStateStore.snapshot(),
            "未 replace() 前 snapshot() 必须是 State.NONE，否则新进程/新类加载会带出脏初值"
        );
    }

    @Test
    void replaceOverwritesSnapshot() {
        DuguV2HudStateStore.State state = fullState();
        DuguV2HudStateStore.replace(state);

        assertEquals(state, DuguV2HudStateStore.snapshot(), "replace() 必须原样写入 snapshot()");
    }

    @Test
    void replaceWithNullFallsBackToNone() {
        DuguV2HudStateStore.replace(fullState());
        DuguV2HudStateStore.replace(null);

        assertEquals(
            DuguV2HudStateStore.State.NONE,
            DuguV2HudStateStore.snapshot(),
            "replace(null) 必须回落到 State.NONE，不能把 null 直接暴露给渲染层"
        );
    }

    /**
     * 核心回归：{@code revealRisk} 没有 expiry 字段、{@code selfRevealed} 是 sticky merge
     * （只会被显式 payload 覆盖，不会随时间自动衰减），这正是 skeleton 里指出的"可无限续命"
     * 字段。clearOnDisconnect() 必须把它们连同其余字段一起清零，不能遗漏。
     */
    @Test
    void clearOnDisconnectResetsStickyRevealAndSelfCureFieldsToNone() {
        DuguV2HudStateStore.replace(fullState());
        assertTrue(DuguV2HudStateStore.snapshot().selfRevealed(), "测试前置：selfRevealed 必须已置 true");
        assertTrue(DuguV2HudStateStore.snapshot().revealRisk() > 0f, "测试前置：revealRisk 必须已 > 0");

        DuguV2HudStateStore.clearOnDisconnect();

        DuguV2HudStateStore.State cleared = DuguV2HudStateStore.snapshot();
        assertEquals(DuguV2HudStateStore.State.NONE, cleared, "clearOnDisconnect() 必须把整体 State 复位为 State.NONE");
        assertFalse(cleared.tainted(), "断线后 tainted 必须归 false");
        assertEquals(0f, cleared.taintIntensity(), "断线后 taintIntensity 必须归零");
        assertEquals("", cleared.taintHint(), "断线后 taintHint 必须清空");
        assertEquals(0f, cleared.revealRisk(), "断线后 revealRisk（无 expiry 字段的粘滞值）必须归零");
        assertEquals(0f, cleared.selfCurePercent(), "断线后 selfCurePercent 必须归零");
        assertFalse(cleared.selfRevealed(), "断线后 sticky selfRevealed 必须回 false，否则下一局会继续显示已自曝");
        assertFalse(cleared.shroudActive(), "断线后 shroudActive 必须归 false");
        assertEquals(0L, cleared.shroudUntilMs(), "断线后 shroudUntilMs 必须归零");
        assertEquals(0f, cleared.qiMaxDecayLoss(), "断线后 qiMaxDecayLoss 必须归零");
        assertEquals(0f, cleared.qiMaxAfter(), "断线后 qiMaxAfter 必须归零");
        assertEquals(0L, cleared.decayExpiryMs(), "断线后 decayExpiryMs 必须归零");
    }

    @Test
    void clearOnDisconnectOnAlreadyNoneSnapshotIsIdempotent() {
        assertEquals(DuguV2HudStateStore.State.NONE, DuguV2HudStateStore.snapshot(), "测试前置：初始必须已是 State.NONE");

        DuguV2HudStateStore.clearOnDisconnect();

        assertEquals(
            DuguV2HudStateStore.State.NONE,
            DuguV2HudStateStore.snapshot(),
            "对已是 State.NONE 的 snapshot 重复调用 clearOnDisconnect() 必须保持幂等，不能抛异常或产生非 NONE 状态"
        );
    }

    @Test
    void clearOnDisconnectDoesNotBlockSubsequentReplaceForNewSession() {
        DuguV2HudStateStore.replace(fullState());
        DuguV2HudStateStore.clearOnDisconnect();

        DuguV2HudStateStore.State newSessionState = new DuguV2HudStateStore.State(
            true, 0.4f, "新局中毒提示", 0.2f, 15f, false, false, 0L, 0f, 0f, 0L);
        DuguV2HudStateStore.replace(newSessionState);

        assertEquals(
            newSessionState,
            DuguV2HudStateStore.snapshot(),
            "回归防线：clearOnDisconnect() 不能变成一次性开关——新 session 收到新毒蛊 v2 payload 后"
                + "正常 replace() 写入必须继续生效"
        );
    }

    private static DuguV2HudStateStore.State fullState() {
        return new DuguV2HudStateStore.State(
            true,        // tainted
            0.8f,        // taintIntensity
            "剧毒攻心",   // taintHint
            0.65f,       // revealRisk（无 expiry，粘滞残留字段之一）
            72.5f,       // selfCurePercent
            true,        // selfRevealed（sticky merge，粘滞残留字段之一）
            true,        // shroudActive
            999_000L,    // shroudUntilMs
            5.5f,        // qiMaxDecayLoss
            40f,         // qiMaxAfter
            999_500L     // decayExpiryMs
        );
    }
}
