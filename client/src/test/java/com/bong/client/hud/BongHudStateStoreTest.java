package com.bong.client.hud;

import com.bong.client.state.NarrationState;
import com.bong.client.state.VisualEffectState;
import com.bong.client.state.ZoneState;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-bughunt-hud-state-session-reset — 覆盖 {@link BongHudStateStore} 的读写与
 * reset 语义。{@code snapshot} 是跨 session 存活的 static 字段，这里锁住：
 * <ul>
 *   <li>replace 写入非空快照后 snapshot() 原样返回</li>
 *   <li>replace(null) 落回 {@link BongHudStateSnapshot#empty()}（不是抛异常/保留旧值）</li>
 *   <li>clear() 把 zoneState/visualEffectState/narrationState 全部复位为 empty/none</li>
 *   <li>clear() 对已经是 empty 的 store 幂等，不抛异常</li>
 *   <li>clear() 后正常 replace() 写入路径不受影响（reset 不是一次性开关）</li>
 * </ul>
 */
class BongHudStateStoreTest {

    @BeforeEach
    void setUp() {
        BongHudStateStore.clear();
    }

    @AfterEach
    void tearDown() {
        BongHudStateStore.clear();
    }

    @Test
    void defaultSnapshotAfterClearIsEmpty() {
        BongHudStateSnapshot snapshot = BongHudStateStore.snapshot();

        assertTrue(snapshot.isEmpty(), "clear() 之后 snapshot 必须是 empty，实际 zoneId="
            + snapshot.zoneState().zoneId() + " visualEffect=" + snapshot.visualEffectState().effectType());
    }

    @Test
    void replaceWritesNonEmptySnapshotAndSnapshotReturnsIt() {
        BongHudStateSnapshot written = BongHudStateSnapshot.create(
            ZoneState.create("blood_valley", "血谷", 0.42, 3, 1_000L),
            NarrationState.empty(),
            VisualEffectState.create("blood_moon", 1.0, 5_000L, 0L)
        );

        BongHudStateStore.replace(written);

        assertSame(written, BongHudStateStore.snapshot(),
            "replace() 后 snapshot() 必须原样返回同一个写入实例，不做隐式拷贝/丢字段");
    }

    @Test
    void replaceWithNullFallsBackToEmptyInsteadOfThrowingOrKeepingStale() {
        BongHudStateStore.replace(BongHudStateSnapshot.create(
            ZoneState.create("blood_valley", "血谷", 0.42, 3, 1_000L),
            NarrationState.empty(),
            VisualEffectState.none()
        ));

        BongHudStateStore.replace(null);

        assertTrue(BongHudStateStore.snapshot().isEmpty(),
            "replace(null) 必须落回 empty snapshot，不能抛异常也不能保留断线前的旧 zoneState");
    }

    @Test
    void clearResetsZoneStateToEmpty() {
        BongHudStateStore.replace(BongHudStateSnapshot.create(
            ZoneState.create("negative_qi_zone", "负灵域", 0.1, 6, 2_000L),
            NarrationState.empty(),
            VisualEffectState.none()
        ));
        assertFalse(BongHudStateStore.snapshot().zoneState().isEmpty(), "测试前置：必须先写入非空 zoneState");

        BongHudStateStore.clear();

        ZoneState resetZoneState = BongHudStateStore.snapshot().zoneState();
        assertTrue(resetZoneState.isEmpty(), "clear() 后 zoneState 必须回到空态，否则旧区域 overlay/atmosphere 会跨 session 残留");
        assertEquals("", resetZoneState.zoneId(),
            "clear() 后 zoneId 必须清空，实际=" + resetZoneState.zoneId());
    }

    @Test
    void clearResetsVisualEffectStateToNone() {
        BongHudStateStore.replace(BongHudStateSnapshot.create(
            ZoneState.empty(),
            NarrationState.empty(),
            VisualEffectState.create("near_death_vignette", 0.8, 10_000L, 0L)
        ));
        assertFalse(BongHudStateStore.snapshot().visualEffectState().isEmpty(), "测试前置：必须先写入非空 visualEffectState");

        BongHudStateStore.clear();

        VisualEffectState resetVisualEffectState = BongHudStateStore.snapshot().visualEffectState();
        assertTrue(resetVisualEffectState.isEmpty(),
            "clear() 后 visualEffectState 必须回到空态，否则旧 HUD tint/相机/FOV 效果会跨 session 残留");
        assertEquals(VisualEffectState.EffectType.NONE, resetVisualEffectState.effectType(),
            "clear() 后 effectType 必须是 NONE，实际=" + resetVisualEffectState.effectType());
    }

    @Test
    void clearResetsNarrationStateToEmpty() {
        BongHudStateStore.replace(BongHudStateSnapshot.create(
            ZoneState.empty(),
            NarrationState.create("broadcast", null, "残留旁白", "narration"),
            VisualEffectState.none()
        ));

        BongHudStateStore.clear();

        NarrationState resetNarrationState = BongHudStateStore.snapshot().narrationState();
        assertTrue(resetNarrationState.isEmpty(),
            "clear() 必须把 narrationState 一并复位，BongHudStateSnapshot.empty() 三字段是同一个原子重置单元，实际 text=\""
                + resetNarrationState.text() + "\"");
    }

    @Test
    void clearOnAlreadyEmptyStoreIsIdempotent() {
        assertTrue(BongHudStateStore.snapshot().isEmpty(), "测试前置：store 应已是 empty（setUp 已 clear）");

        BongHudStateStore.clear();
        BongHudStateStore.clear();

        assertTrue(BongHudStateStore.snapshot().isEmpty(), "对已空 store 重复 clear() 不应抛异常或改变状态");
    }

    @Test
    void replaceAfterClearStillWritesNormally() {
        BongHudStateStore.clear();

        BongHudStateSnapshot written = BongHudStateSnapshot.create(
            ZoneState.create("qingyun_peaks", "青云残峰", 0.9, 0, 3_000L),
            NarrationState.empty(),
            VisualEffectState.none()
        );
        BongHudStateStore.replace(written);

        assertSame(written, BongHudStateStore.snapshot(),
            "clear() 不能变成一次性开关——之后正常的 replace() 写入路径必须继续生效，供在线收到新 zone_info 时更新 HUD");
    }
}
