package com.bong.client.hud;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-combat-skill-feedback-bridges-v1 P4 @hive blocker+major 修复：
 * 验证 AnqiHudStateStore 的按维度独立存储、并发合并语义和 tick 守序。
 *
 * <p>测试矩阵覆盖：
 * <ul>
 *   <li>echo + charge 并存（blocker 核心：后到 charge 不清零 echo）</li>
 *   <li>charge + echo 顺序反转两维并存</li>
 *   <li>echo + charge + abrasion 三维并存</li>
 *   <li>单维度 expiry（echo 过期后 snapshot 只剩 charge）</li>
 *   <li>abrasion 过期 echo/charge 不受影响</li>
 *   <li>所有维度过期后 active() = false</li>
 *   <li>clear() 重置所有维度</li>
 *   <li>snapshot(now) 和 snapshot() 无参两种调用形式</li>
 *   <li>aim 维度保留（server 不发，始终为 0）</li>
 *   <li>updateAim() 接口正常写入</li>
 *   <li>Planner 对三维并存 state 产出三路命令</li>
 *   <li>Planner 对 echo+charge 并存产出两路命令</li>
 *   <li>[major] tick 守序：同维度乱序旧包被丢弃（echo/charge/abrasion 各自验证）</li>
 *   <li>[major] tick 守序：同维度相同 tick 第二次 update 被接受（允许 >=）</li>
 *   <li>[major] tick 守序：同 tick 不同维度均被接受（同帧多维度）</li>
 * </ul>
 */
class AnqiHudStateStoreTest {

    private static final long BASE_NOW = 1_000_000L; // 固定时基，避免 System.currentTimeMillis 竞争
    private static final long DUR      = 2_000L;      // 显示时长 2s

    @BeforeEach
    void setUp() {
        AnqiHudStateStore.clear();
    }

    @AfterEach
    void tearDown() {
        AnqiHudStateStore.clear();
    }

    // ─── blocker 核心：echo + charge 并存 ────────────────────────────

    @Test
    void echoThenChargeBothPresentInSnapshot() {
        // 先发 echo，再发 charge——旧 replace 语义下 charge 会清零 echo；新 merge 语义应并存
        AnqiHudStateStore.updateEcho(3, BASE_NOW, DUR, 10L);
        AnqiHudStateStore.updateCharge(0.5f, BASE_NOW, DUR, 11L);

        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);

        assertEquals(3, state.echoCount(),
            "echo 应未被 charge update 清零，期望 echoCount=3；实际=" + state.echoCount());
        assertEquals(0.5f, state.chargeProgress(), 0.001f,
            "charge 维度应被写入，期望 chargeProgress=0.5；实际=" + state.chargeProgress());
    }

    @Test
    void chargeThenEchoBothPresentInSnapshot() {
        // 顺序反转：先 charge 再 echo，两者仍应并存
        AnqiHudStateStore.updateCharge(0.8f, BASE_NOW, DUR, 5L);
        AnqiHudStateStore.updateEcho(2, BASE_NOW, DUR, 6L);

        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);

        assertEquals(2, state.echoCount(),
            "echo 应未被 charge update 清零，期望 echoCount=2；实际=" + state.echoCount());
        assertEquals(0.8f, state.chargeProgress(), 0.001f,
            "charge 维度应被写入，期望 chargeProgress=0.8；实际=" + state.chargeProgress());
    }

    @Test
    void echoChargeAbrasionThreeDimsAllPresentInSnapshot() {
        // 三维全部发——全部应在 snapshot 中并存
        AnqiHudStateStore.updateEcho(5, BASE_NOW, DUR, 10L);
        AnqiHudStateStore.updateCharge(0.6f, BASE_NOW, DUR, 10L);
        AnqiHudStateStore.updateAbrasion("pocket_pouch", 45.0f, BASE_NOW, DUR, 10L);

        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);

        assertEquals(5, state.echoCount(),
            "三维并存：echoCount 必须为 5；实际=" + state.echoCount());
        assertEquals(0.6f, state.chargeProgress(), 0.001f,
            "三维并存：chargeProgress 必须为 0.6；实际=" + state.chargeProgress());
        assertTrue(state.hasAbrasionContainer(),
            "三维并存：abrasion 维度必须存在；实际=" + state.abrasionContainer());
        assertEquals("pocket_pouch", state.abrasionContainer(),
            "三维并存：abrasionContainer 必须为 pocket_pouch；实际=" + state.abrasionContainer());
        assertEquals(45.0f, state.abrasionQiPayload(), 0.01f,
            "三维并存：abrasionQiPayload 必须为 45.0；实际=" + state.abrasionQiPayload());
    }

    // ─── 维度独立 expiry ──────────────────────────────────────────────

    @Test
    void expiredEchoDroppedWhileChargeRemains() {
        // echo 设短 expiry 过期，charge 仍有效——snapshot 只剩 charge
        long shortDur = 100L;
        AnqiHudStateStore.updateEcho(3, BASE_NOW, shortDur, 10L);
        AnqiHudStateStore.updateCharge(0.7f, BASE_NOW, DUR, 10L);

        long afterEchoExpiry = BASE_NOW + shortDur + 1L; // echo 已过期
        AnqiHudState state = AnqiHudStateStore.snapshot(afterEchoExpiry);

        assertEquals(0, state.echoCount(),
            "echo 过期后 echoCount 应为 0（已从 snapshot 剔除）；实际=" + state.echoCount());
        assertEquals(0.7f, state.chargeProgress(), 0.001f,
            "echo 过期不影响 charge，chargeProgress 仍应为 0.7；实际=" + state.chargeProgress());
    }

    @Test
    void expiredAbrasionDroppedWhileEchoAndChargeRemain() {
        // abrasion 先过期，echo/charge 仍有效
        long shortDur = 50L;
        AnqiHudStateStore.updateAbrasion("jade_tube", 12.0f, BASE_NOW, shortDur, 1L);
        AnqiHudStateStore.updateEcho(4, BASE_NOW, DUR, 1L);
        AnqiHudStateStore.updateCharge(0.3f, BASE_NOW, DUR, 1L);

        long afterAbrasionExpiry = BASE_NOW + shortDur + 1L;
        AnqiHudState state = AnqiHudStateStore.snapshot(afterAbrasionExpiry);

        assertFalse(state.hasAbrasionContainer(),
            "abrasion 过期后 hasAbrasionContainer 应为 false；实际=" + state.abrasionContainer());
        assertEquals(0f, state.abrasionQiPayload(), 0.001f,
            "abrasion 过期后 qiPayload 应为 0；实际=" + state.abrasionQiPayload());
        assertEquals(4, state.echoCount(),
            "abrasion 过期不影响 echo，echoCount 仍应为 4；实际=" + state.echoCount());
        assertEquals(0.3f, state.chargeProgress(), 0.001f,
            "abrasion 过期不影响 charge，chargeProgress 仍应为 0.3；实际=" + state.chargeProgress());
    }

    @Test
    void allDimsExpiredSnapshotIsEffectivelyEmpty() {
        // 所有维度都过期时，snapshot 字段值等同 empty（active() = false）
        long shortDur = 10L;
        AnqiHudStateStore.updateEcho(2, BASE_NOW, shortDur, 1L);
        AnqiHudStateStore.updateCharge(0.5f, BASE_NOW, shortDur, 1L);

        long afterAll = BASE_NOW + shortDur + 1L;
        AnqiHudState state = AnqiHudStateStore.snapshot(afterAll);

        assertFalse(state.active(afterAll),
            "所有维度过期后 active() 应为 false；实际 expiresAt=" + state.expiresAtMillis());
        assertEquals(0, state.echoCount(),
            "所有维度过期后 echoCount 应为 0；实际=" + state.echoCount());
        assertEquals(0f, state.chargeProgress(), 0.001f,
            "所有维度过期后 chargeProgress 应为 0；实际=" + state.chargeProgress());
    }

    // ─── clear 重置 ───────────────────────────────────────────────────

    @Test
    void clearResetsAllDimensions() {
        AnqiHudStateStore.updateEcho(3, BASE_NOW, DUR, 1L);
        AnqiHudStateStore.updateCharge(0.5f, BASE_NOW, DUR, 1L);
        AnqiHudStateStore.updateAbrasion("bag", 10f, BASE_NOW, DUR, 1L);

        AnqiHudStateStore.clear();
        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);

        assertEquals(AnqiHudState.empty(), state,
            "clear() 后 snapshot 应等同 empty()；实际=" + state);
    }

    // ─── snapshot() 无参形式 ──────────────────────────────────────────

    @Test
    void snapshotNoArgUsesCurrentTime() {
        // snapshot() 无参形式：刚 update 的维度应 active
        AnqiHudStateStore.updateEcho(2, System.currentTimeMillis(), DUR, 1L);
        AnqiHudState state = AnqiHudStateStore.snapshot(); // 无参，uses currentTimeMillis

        assertEquals(2, state.echoCount(),
            "snapshot() 无参形式：刚 update 的 echo 应 active，echoCount=2；实际=" + state.echoCount());
    }

    // ─── aim 维度保留（server 不发，始终为 0）──────────────────────────

    @Test
    void aimDimNotUpdatedByHandlerRemainsEmpty() {
        // aim 维度没有 update → snapshot 中 aimProgress=0，不影响其他维度
        AnqiHudStateStore.updateEcho(1, BASE_NOW, DUR, 1L);
        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);

        assertEquals(0f, state.aimProgress(), 0.001f,
            "aim 维度未被 server update，aimProgress 应始终为 0；实际=" + state.aimProgress());
        assertEquals(1, state.echoCount(),
            "aim 不影响 echo，echoCount 应为 1；实际=" + state.echoCount());
    }

    @Test
    void updateAimWorks() {
        // updateAim() 接口本身可被测试（即使 server 当前不发）
        AnqiHudStateStore.updateAim(0.6f, BASE_NOW, DUR, 1L);
        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);

        assertEquals(0.6f, state.aimProgress(), 0.001f,
            "updateAim 应正确写入 aimProgress=0.6；实际=" + state.aimProgress());
    }

    // ─── Planner 对三维并存 state 产出三路命令 ─────────────────────────

    @Test
    void plannerBuildsCommandsForAllThreeDimsConcurrently() {
        // 三维并存 → buildCommands 产出 echo + charge + abrasion 三路命令
        AnqiHudStateStore.updateEcho(3, BASE_NOW, DUR, 10L);
        AnqiHudStateStore.updateCharge(0.6f, BASE_NOW, DUR, 10L);
        AnqiHudStateStore.updateAbrasion("pocket_pouch", 50.0f, BASE_NOW, DUR, 10L);

        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);
        List<HudRenderCommand> commands = AnqiHudPlanner.buildCommands(state, BASE_NOW, 800, 600);

        assertFalse(commands.isEmpty(),
            "三维并存 state 下 AnqiHudPlanner 必须产生非空命令列表；实际=空");

        boolean hasEcho = commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("echo 3"));
        assertTrue(hasEcho,
            "三维并存：Planner 应产生 echo 相关文本命令（含'echo 3'）；commands=" + commands);

        boolean hasCharge = commands.stream().anyMatch(cmd ->
            cmd.layer() == HudRenderLayer.CAST_BAR && cmd.height() == 5);
        assertTrue(hasCharge,
            "三维并存：Planner 应产生 charge bar 命令（CAST_BAR, height=5）；commands=" + commands);

        boolean hasAbrasion = commands.stream().anyMatch(cmd ->
            cmd.isText() && cmd.text().contains("pocket_pouch"));
        assertTrue(hasAbrasion,
            "三维并存：Planner 应产生 abrasion 文本命令（含 pocket_pouch）；commands=" + commands);
    }

    @Test
    void plannerBuildsEchoAndChargeCommandsWhenBothActive() {
        // echo + charge 并存（没有 abrasion），Planner 同时产生两路命令
        AnqiHudStateStore.updateEcho(2, BASE_NOW, DUR, 5L);
        AnqiHudStateStore.updateCharge(0.4f, BASE_NOW, DUR, 6L);

        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);
        List<HudRenderCommand> commands = AnqiHudPlanner.buildCommands(state, BASE_NOW, 800, 600);

        boolean hasEcho = commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("echo"));
        assertTrue(hasEcho,
            "echo+charge 并存：Planner 应产生 echo 命令；commands=" + commands);

        boolean hasCharge = commands.stream().anyMatch(cmd ->
            cmd.layer() == HudRenderLayer.CAST_BAR);
        assertTrue(hasCharge,
            "echo+charge 并存：Planner 应产生 charge bar 命令；commands=" + commands);
    }

    // ─── [major] tick 守序：同维度乱序旧包丢弃 ───────────────────────────

    @Test
    void staleTickEchoIsDropped() {
        // 先 feed tick=5 echo=3，再 feed tick=2 echo=0（乱序旧包）→ snapshot 仍为 echoCount=3
        AnqiHudStateStore.updateEcho(3, BASE_NOW, DUR, 5L);
        AnqiHudStateStore.updateEcho(0, BASE_NOW, DUR, 2L); // stale：tick=2 < lastTick=5，丢弃

        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);

        assertEquals(3, state.echoCount(),
            "乱序旧包（echo tick=2 < lastTick=5）应被丢弃，echoCount 仍应为 3；实际=" + state.echoCount());
    }

    @Test
    void staleTickChargeIsDropped() {
        // charge 维度 tick 守序：旧 tick 包被拒绝
        AnqiHudStateStore.updateCharge(0.9f, BASE_NOW, DUR, 10L);
        AnqiHudStateStore.updateCharge(0.1f, BASE_NOW, DUR, 3L); // stale：tick=3 < lastTick=10

        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);

        assertEquals(0.9f, state.chargeProgress(), 0.001f,
            "charge 乱序旧包（tick=3 < lastTick=10）应被丢弃；实际=" + state.chargeProgress());
    }

    @Test
    void staleTickAbrasionIsDropped() {
        // abrasion 维度 tick 守序
        AnqiHudStateStore.updateAbrasion("quiver", 30.0f, BASE_NOW, DUR, 20L);
        AnqiHudStateStore.updateAbrasion("old_bag", 5.0f, BASE_NOW, DUR, 8L); // stale

        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);

        assertEquals("quiver", state.abrasionContainer(),
            "abrasion 乱序旧包（tick=8 < lastTick=20）应被丢弃，container 仍为 quiver；实际=" + state.abrasionContainer());
        assertEquals(30.0f, state.abrasionQiPayload(), 0.01f,
            "abrasion 乱序旧包丢弃后 qiPayload 仍应为 30.0；实际=" + state.abrasionQiPayload());
    }

    // ─── [major] tick 守序：相同 tick 允许 >= ─────────────────────────────

    @Test
    void sameTickSameDimIsAccepted() {
        // 同 tick 相同维度第二次 update 被接受（tick >= lastTick 允许 =）
        AnqiHudStateStore.updateEcho(3, BASE_NOW, DUR, 5L);
        AnqiHudStateStore.updateEcho(7, BASE_NOW, DUR, 5L); // 相同 tick，应覆盖

        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);

        assertEquals(7, state.echoCount(),
            "相同 tick(=5) 第二次 update 应被接受（允许 >=），echoCount 应为 7；实际=" + state.echoCount());
    }

    // ─── [major] tick 守序：同 tick 不同维度均被接受 ─────────────────────

    @Test
    void sameTickDifferentDimsBothAccepted() {
        // 同 tick 不同维度均被接受（同帧 server 可能发多 kind 同 tick）
        AnqiHudStateStore.updateEcho(5, BASE_NOW, DUR, 42L);
        AnqiHudStateStore.updateCharge(0.5f, BASE_NOW, DUR, 42L);
        AnqiHudStateStore.updateAbrasion("hand_slot", 18.5f, BASE_NOW, DUR, 42L);

        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);

        assertEquals(5, state.echoCount(),
            "同帧同 tick：echoCount 必须为 5；实际=" + state.echoCount());
        assertEquals(0.5f, state.chargeProgress(), 0.001f,
            "同帧同 tick：chargeProgress 必须为 0.5；实际=" + state.chargeProgress());
        assertEquals("hand_slot", state.abrasionContainer(),
            "同帧同 tick：abrasionContainer 必须为 hand_slot；实际=" + state.abrasionContainer());
        assertEquals(18.5f, state.abrasionQiPayload(), 0.01f,
            "同帧同 tick：abrasionQiPayload 必须为 18.5；实际=" + state.abrasionQiPayload());
    }

    @Test
    void staleTickDoesNotAffectOtherDimensions() {
        // 验证：某维度乱序包被丢弃时，其他维度不受影响
        AnqiHudStateStore.updateEcho(3, BASE_NOW, DUR, 10L);
        AnqiHudStateStore.updateCharge(0.7f, BASE_NOW, DUR, 10L);
        AnqiHudStateStore.updateEcho(0, BASE_NOW, DUR, 2L); // stale echo，被丢弃

        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);

        assertEquals(3, state.echoCount(),
            "echo stale 包被丢弃，echoCount 仍应为 3；实际=" + state.echoCount());
        assertEquals(0.7f, state.chargeProgress(), 0.001f,
            "echo stale 包丢弃不影响 charge，chargeProgress 仍应为 0.7；实际=" + state.chargeProgress());
    }

    // ─── AV 里程碑：multishot 维度（多发齐射弹数）─────────────────────────

    @Test
    void updateMultiShotWritesIndependentDimension() {
        // multishot 维度与 echo 独立：复用 echo_count 承载但走 MULTISHOT_SLOT，不串维度
        AnqiHudStateStore.updateMultiShot(5, BASE_NOW, DUR, 1L);
        AnqiHudStateStore.updateEcho(2, BASE_NOW, DUR, 1L);

        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);

        assertEquals(5, state.multiShotCount(),
            "multishot 维度应独立写入 multiShotCount=5；实际=" + state.multiShotCount());
        assertEquals(2, state.echoCount(),
            "multishot 与 echo 互不干扰，echoCount 仍应为 2；实际=" + state.echoCount());
    }

    @Test
    void expiredMultiShotDroppedWhileEchoRemains() {
        long shortDur = 60L;
        AnqiHudStateStore.updateMultiShot(7, BASE_NOW, shortDur, 1L);
        AnqiHudStateStore.updateEcho(4, BASE_NOW, DUR, 1L);

        long afterMultiShotExpiry = BASE_NOW + shortDur + 1L;
        AnqiHudState state = AnqiHudStateStore.snapshot(afterMultiShotExpiry);

        assertEquals(0, state.multiShotCount(),
            "multishot 过期后应从 snapshot 剔除（multiShotCount=0）；实际=" + state.multiShotCount());
        assertEquals(4, state.echoCount(),
            "multishot 过期不影响 echo，echoCount 仍应为 4；实际=" + state.echoCount());
    }

    @Test
    void staleTickMultiShotIsDropped() {
        AnqiHudStateStore.updateMultiShot(6, BASE_NOW, DUR, 12L);
        AnqiHudStateStore.updateMultiShot(0, BASE_NOW, DUR, 4L); // stale：tick=4 < lastTick=12

        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);

        assertEquals(6, state.multiShotCount(),
            "multishot 乱序旧包（tick=4 < lastTick=12）应被丢弃，multiShotCount 仍应为 6；实际="
                + state.multiShotCount());
    }

    @Test
    void clearResetsMultiShotDimension() {
        AnqiHudStateStore.updateMultiShot(8, BASE_NOW, DUR, 1L);
        AnqiHudStateStore.clear();
        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);
        assertEquals(0, state.multiShotCount(),
            "clear() 后 multiShotCount 应为 0；实际=" + state.multiShotCount());
    }

    @Test
    void plannerBuildsMultiShotIndicatorWhenActive() {
        // multishot 维度有值 → Planner 产出齐射刻度块（CARRIER 层，count 个 width=4 块）
        AnqiHudStateStore.updateMultiShot(5, BASE_NOW, DUR, 1L);
        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);
        List<HudRenderCommand> commands = AnqiHudPlanner.buildCommands(state, BASE_NOW, 800, 600);

        long blockCount = commands.stream()
            .filter(cmd -> cmd.layer() == HudRenderLayer.CARRIER && cmd.width() == 4 && cmd.height() == 3)
            .count();
        assertEquals(5L, blockCount,
            "multishot=5 应产出 5 个齐射刻度块（width=4,height=3）；实际块数=" + blockCount
                + " commands=" + commands);
    }

    @Test
    void multiShotInactiveProducesNoIndicator() {
        // 未激活（无 multishot）→ 不产出齐射指示（HUD 极简：未激活隐藏）
        AnqiHudStateStore.updateEcho(2, BASE_NOW, DUR, 1L);
        AnqiHudState state = AnqiHudStateStore.snapshot(BASE_NOW);
        List<HudRenderCommand> commands = AnqiHudPlanner.buildCommands(state, BASE_NOW, 800, 600);

        boolean hasMultiShotBlock = commands.stream()
            .anyMatch(cmd -> cmd.layer() == HudRenderLayer.CARRIER && cmd.width() == 4 && cmd.height() == 3);
        assertFalse(hasMultiShotBlock,
            "multishot 未激活时不应产出齐射刻度块（HUD 极简未激活隐藏）；commands=" + commands);
    }
}
