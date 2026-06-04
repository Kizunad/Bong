package com.bong.client.hud;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-combat-skill-feedback-bridges-v1 P4 @hive blocker 修复：
 * 验证 AnqiHudStateStore 的按维度独立存储和并发合并语义。
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
}
