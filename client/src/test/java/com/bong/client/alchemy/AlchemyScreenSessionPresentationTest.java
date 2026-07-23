package com.bong.client.alchemy;

import bong.Envelope;
import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.alchemy.state.AlchemySessionStore;
import com.bong.client.hud.AlchemyProgressHudPlanner;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.network.ProtoServerDataBridge;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerPayloadParseResult;
import com.bong.client.network.alchemy.AlchemySessionHandler;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class AlchemyScreenSessionPresentationTest {
    private static final BlockPos FURNACE_POS = new BlockPos(12, 64, -8);
    private static final Path RUST_PROTO_FIXTURE_DIR = Path.of("..", "proto", "fixtures");
    private static final String ACTIVE_SESSION_FIXTURE = "alchemy_session_active_v1.pb";
    private static final String FINISHED_SESSION_FIXTURE = "alchemy_session_finished_v1.pb";

    private final List<String> sentPayloads = new ArrayList<>();

    @BeforeEach
    void setUp() {
        AlchemySessionStore.resetForTests();
        AlchemyFurnaceStore.resetForTests();
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sentPayloads.add(new String(payload, StandardCharsets.UTF_8))
        );
    }

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        AlchemySessionStore.resetForTests();
        AlchemyFurnaceStore.resetForTests();
    }

    @Test
    void openedScreenRefreshesTerminalGuidanceAndKeepsCentralHudHidden() {
        AlchemyFurnaceStore.replace(furnace(false));
        AlchemyScreen screen = new AlchemyScreen(FURNACE_POS);
        screen.attachSessionListenerForTests();
        AlchemySessionPresentationPlanner.Presentation awaiting =
            screen.sessionPresentationForTests();
        assertTrue(awaiting.idle(),
            "空炉且尚未收到 session 时必须显示真正 idle，而不是伪造会话占用");

        AlchemySessionStore.replace(finishedSession());

        AlchemySessionPresentationPlanner.Presentation view = screen.sessionPresentationForTests();
        assertNotNull(view, "已打开 screen 必须收到 AlchemySessionStore 网络更新");
        assertTrue(view.terminal(),
            "空炉收到完整 inactive guidance 后必须进入 terminal 呈现");
        assertFalse(view.active());
        assertEquals("§a已结束", view.statusText(),
            "终态文案应保留服务端 status_label 且不重复追加相同状态");
        assertEquals("§f177 / 240t", view.progressText());
        assertEquals("§e0.73 / 0.61", view.temperatureText());
        assertEquals("§713.5 / 18.0", view.qiText());
        assertTrue(view.detailLines().contains("§a✓ §7t12 (+3) §fci_she_hao×2 + ling_shui×1"));
        assertTrue(view.detailLines().contains("§c× §7t101 (+9) §fdan_sha×3"));
        assertTrue(view.detailLines().contains("§e○ §7t190 (+4) §f（无投料）"));
        assertTrue(view.detailLines().contains("§7干预：§7AdjustTemp(0.73)"));
        assertTrue(AlchemyProgressHudPlanner.buildCommands(320, 180, 2_000L).stream()
                .noneMatch(command -> command.layer() == HudRenderLayer.PROCESSING_HUD),
            "terminal snapshot 为 inactive，会保留炉内 guidance，但中央 HUD 必须继续隐藏");

        screen.detachSessionListenerForTests();
    }

    @Test
    void rustProducedActiveSnapshotRefreshesAlreadyOpenRealScreenThroughProductionWire() {
        AlchemyScreen screen = new AlchemyScreen(FURNACE_POS);
        screen.attachSessionListenerForTests();
        AlchemySessionPresentationPlanner.Presentation awaiting =
            screen.sessionPresentationForTests();
        assertTrue(awaiting.idle(),
            "screen 先打开且尚未收到任何快照时必须显示 idle");

        ServerDataDispatch sessionDispatch = dispatchRustProductionSessionFixture(
            ACTIVE_SESSION_FIXTURE
        );
        assertTrue(sessionDispatch.handled(),
            "Rust 生产的 active fixture 必须经 production bridge/handler 入 store，实际 log="
                + sessionDispatch.logMessage());

        AlchemySessionPresentationPlanner.Presentation view = screen.sessionPresentationForTests();
        assertTrue(view.active(),
            "Rust fixture 必须经 production proto bridge 刷新已打开的真实 AlchemyScreen");
        assertEquals("§f44 / 180t", view.progressText());
        assertEquals("§e0.58 / 0.62", view.temperatureText());
        assertEquals("§77.3 / 12.5", view.qiText());
        assertTrue(view.detailLines().contains("§7AdjustTemp(0.58)"));
        assertEquals(List.of(), view.flashingStageSlots(),
            "fixture 当前两个未完成 stage 都不在 elapsed=44 的开放窗口内");

        screen.detachSessionListenerForTests();
    }

    @Test
    void rustProducedFinishedSnapshotSurvivesNormalEmptyFurnaceOrderingOnOpenScreen() {
        AlchemyFurnaceStore.replace(furnace(false));
        AlchemyScreen screen = new AlchemyScreen(FURNACE_POS);
        screen.attachSessionListenerForTests();
        assertTrue(screen.sessionPresentationForTests().idle());

        ServerDataDispatch sessionDispatch = dispatchRustProductionSessionFixture(
            FINISHED_SESSION_FIXTURE
        );
        assertTrue(sessionDispatch.handled(),
            "Rust 生产的 finished fixture 必须经 production bridge/handler 入 store，实际 log="
                + sessionDispatch.logMessage());

        AlchemySessionPresentationPlanner.Presentation view = screen.sessionPresentationForTests();
        assertTrue(view.terminal(),
            "empty furnace → completed session 的生产顺序必须保留终态权威 guidance");
        assertFalse(view.active());
        assertEquals("§a已结束", view.statusText());
        assertEquals("§f44 / 180t", view.progressText());
        assertEquals("§e0.58 / 0.62", view.temperatureText());
        assertEquals("§77.3 / 12.5", view.qiText());
        assertTrue(view.detailLines().contains(
            "§a✓ §7t0 (+0) §fci_she_hao×2 + ling_shui×1"));
        assertTrue(view.detailLines().contains("§c× §7t40 (+6) §fdan_sha×3"));
        assertTrue(view.detailLines().contains("§e○ §7t120 (+4) §f（无投料）"));
        assertTrue(AlchemyProgressHudPlanner.buildCommands(320, 180, 2_000L).stream()
                .noneMatch(command -> command.layer() == HudRenderLayer.PROCESSING_HUD),
            "finished fixture 为 inactive，真实 screen 保留 guidance 时中央 HUD 仍必须隐藏");

        screen.detachSessionListenerForTests();
    }

    @Test
    void openedScreenRefreshesEveryActiveSnapshotIncludingStageAndInterventionChanges() {
        AlchemyFurnaceStore.replace(furnace(true));
        AlchemyScreen screen = new AlchemyScreen(FURNACE_POS);
        List<AlchemySessionPresentationPlanner.Presentation> refreshes = new ArrayList<>();
        screen.setSessionPresentationObserverForTests(refreshes::add);
        screen.attachSessionListenerForTests();
        int refreshesAfterAttach = refreshes.size();

        AlchemySessionStore.replace(activeSessionAtStart());

        assertEquals(refreshesAfterAttach + 1, refreshes.size(),
            "首次异步 active snapshot 必须让已打开 screen 恰好刷新一次");
        AlchemySessionPresentationPlanner.Presentation initial = screen.sessionPresentationForTests();
        assertTrue(initial.active());
        assertEquals("§f20 / 100t", initial.progressText());
        assertEquals("§e0.50 / 0.55", initial.temperatureText());
        assertEquals("§72.0 / 5.0", initial.qiText());
        assertEquals(
            List.of("§7干预", "§7AdjustTemp(0.50)"),
            initial.detailLines(),
            "首次 active snapshot 的 intervention guidance 必须直接进入已打开 screen"
        );
        assertEquals(List.of(0), initial.flashingStageSlots(),
            "首次 active snapshot 必须让当前窗口 stage 进入真实 slot 闪烁规划");

        AlchemySessionStore.replace(activeSessionAfterFeedAndIntervention());

        assertEquals(refreshesAfterAttach + 2, refreshes.size(),
            "后续 active stage/intervention snapshot 必须再次恰好刷新一次");
        AlchemySessionPresentationPlanner.Presentation updated = screen.sessionPresentationForTests();
        assertTrue(updated.active());
        assertEquals("§f46 / 100t", updated.progressText());
        assertEquals("§e0.57 / 0.55", updated.temperatureText());
        assertEquals("§73.5 / 5.0", updated.qiText());
        assertEquals(
            List.of("§7干预", "§7FeedStage(0)", "§7InjectQi(3.5)"),
            updated.detailLines(),
            "active-to-active 更新不得冻结旧 intervention guidance"
        );
        assertEquals(List.of(1), updated.flashingStageSlots(),
            "后续 active snapshot 必须清掉已完成 stage，并闪烁新窗口对应 slot");
        assertEquals(2, AlchemySessionStore.snapshot().stages().size());
        assertTrue(AlchemySessionStore.snapshot().stages().get(0).completed(),
            "后续 active snapshot 的 stage 完成态必须保留给 screen 的 stage-flash 消费链");
        assertEquals(40, AlchemySessionStore.snapshot().stages().get(1).atTick());
        assertEquals(8, AlchemySessionStore.snapshot().stages().get(1).window());
        assertEquals("dan_sha×3", AlchemySessionStore.snapshot().stages().get(1).summary());
        assertFalse(initial.equals(updated),
            "active-to-active 更新必须替换 presentation，不能保留首次网络快照");

        screen.detachSessionListenerForTests();
    }

    @Test
    void emptyFurnaceAndSessionResetClearTerminalPresentation() {
        AlchemyFurnaceStore.replace(furnace(true));
        AlchemyScreen screen = new AlchemyScreen(FURNACE_POS);
        screen.attachSessionListenerForTests();
        AlchemySessionStore.replace(finishedSession());
        assertTrue(screen.sessionPresentationForTests().terminal());

        AlchemyFurnaceStore.replace(furnace(false));
        AlchemySessionStore.replace(AlchemySessionStore.Snapshot.empty());

        AlchemySessionPresentationPlanner.Presentation cleared = screen.sessionPresentationForTests();
        assertTrue(cleared.idle(),
            "empty furnace/session reset 必须清除旧终态");
        assertEquals("§8未起炉", cleared.statusText());
        assertEquals("§70 / 0t", cleared.progressText());
        assertEquals("", cleared.temperatureText());
        assertEquals("", cleared.qiText());
        assertEquals(List.of("§7干预"), cleared.detailLines(),
            "empty session 不得残留 stage 或 intervention guidance");

        screen.detachSessionListenerForTests();
    }

    @Test
    void repeatedAttachIsIdempotentAndRemovedScreenReceivesNoLaterUpdates() {
        AlchemyFurnaceStore.replace(furnace(true));
        AlchemyScreen screen = new AlchemyScreen(FURNACE_POS);
        int[] refreshCount = {0};
        screen.setSessionPresentationObserverForTests(ignored -> refreshCount[0]++);
        screen.attachSessionListenerForTests();
        screen.attachSessionListenerForTests();
        screen.attachSessionListenerForTests();
        assertEquals(1, AlchemySessionStore.listenerCountForTests(),
            "重复 init/build/resize 等价 attach 必须只保留一个 session listener");
        int refreshesBeforeUpdate = refreshCount[0];

        AlchemySessionStore.replace(finishedSession());
        assertEquals(refreshesBeforeUpdate + 1, refreshCount[0],
            "单次 store update 必须恰好刷新一次，不能因重复 attach 倍增");
        AlchemySessionPresentationPlanner.Presentation retained = screen.sessionPresentationForTests();
        assertTrue(retained.terminal(),
            "幂等 listener 仍必须把 store 更新送到已打开 screen");

        screen.detachSessionListenerForTests();
        assertEquals(0, AlchemySessionStore.listenerCountForTests(),
            "removed/detach 必须从 store 解绑 listener");
        screen.attachSessionListenerForTests();
        assertEquals(0, AlchemySessionStore.listenerCountForTests(),
            "removed 后不得 reattach zombie listener");
        int refreshesAfterRemoval = refreshCount[0];
        AlchemySessionStore.replace(activeSession());

        assertEquals(refreshesAfterRemoval, refreshCount[0],
            "removed screen 后续 store update 的刷新次数必须为零");
        assertEquals(retained, screen.sessionPresentationForTests(),
            "removed 后不得收到后续 store 更新");
    }

    @Test
    void terminalSessionStillSendsRealTakeBackAction() {
        AlchemyFurnaceStore.replace(furnace(true));
        AlchemySessionStore.replace(finishedSession());
        AlchemyScreen screen = new AlchemyScreen(FURNACE_POS);

        assertTrue(screen.takeBackForTests(), "T take-back action 必须继续由 AlchemyScreen 消费");
        assertEquals(
            List.of("{\"type\":\"alchemy_take_back\",\"v\":1,\"furnace_pos\":[12,64,-8],\"slot_idx\":0}"),
            sentPayloads,
            "terminal session 的 T 必须走真实 ClientRequestSender alchemy_take_back seam"
        );
    }

    private static ServerDataDispatch dispatchRustProductionSessionFixture(String fileName) {
        ServerDataEnvelope parsed = decodeThroughProductionBridge(
            readRustProductionFixture(fileName)
        );
        assertEquals("alchemy_session", parsed.type(),
            "共享 Rust fixture 必须是完整 ServerDataEnvelope.alchemy_session");
        return new AlchemySessionHandler().handle(parsed);
    }

    private static byte[] readRustProductionFixture(String fileName) {
        Path fixturePath = RUST_PROTO_FIXTURE_DIR.resolve(fileName);
        assertTrue(Files.isRegularFile(fixturePath),
            "Rust 生产 protobuf fixture 必须存在：" + fixturePath.toAbsolutePath());
        try {
            return Files.readAllBytes(fixturePath);
        } catch (IOException error) {
            throw new AssertionError(
                "读取 Rust 生产 protobuf fixture 失败：" + fixturePath.toAbsolutePath(),
                error
            );
        }
    }

    private static ServerDataEnvelope decodeThroughProductionBridge(byte[] envelopeBytes) {
        ProtoServerDataBridge.BridgeResult bridged = ProtoServerDataBridge.bridge(envelopeBytes);
        assertTrue(bridged.isSuccess(),
            "proto→legacyJson 桥接应成功，实际 error=" + bridged.errorMessage());

        String legacyJson = bridged.legacyJson();
        ServerPayloadParseResult parsed = ServerDataEnvelope.parse(
            legacyJson,
            legacyJson.getBytes(StandardCharsets.UTF_8).length
        );
        assertTrue(parsed.isSuccess(),
            "桥输出的 legacyJson 应能被解析，实际 error=" + parsed.errorMessage()
                + "，legacyJson=" + legacyJson);
        return parsed.envelope();
    }

    private static AlchemyFurnaceStore.Snapshot furnace(boolean hasSession) {
        return new AlchemyFurnaceStore.Snapshot(
            FURNACE_POS, 2, 88.0f, 100.0f, "Azure", hasSession
        );
    }

    private static AlchemySessionStore.Snapshot finishedSession() {
        return new AlchemySessionStore.Snapshot(
            "finished_contract_recipe",
            false,
            177,
            240,
            0.73f,
            0.61f,
            0.07f,
            13.5,
            18.0,
            "已结束",
            List.of(
                new AlchemySessionStore.StageHint(
                    12, 3, "ci_she_hao×2 + ling_shui×1", true, false),
                new AlchemySessionStore.StageHint(101, 9, "dan_sha×3", false, true),
                new AlchemySessionStore.StageHint(190, 4, "", false, false)
            ),
            List.of("§7AdjustTemp(0.73)", "§7InjectQi(13.5)")
        );
    }

    private static AlchemySessionStore.Snapshot activeSession() {
        return activeSessionAtStart();
    }

    private static AlchemySessionStore.Snapshot activeSessionAtStart() {
        return new AlchemySessionStore.Snapshot(
            "active_contract_recipe",
            true,
            20,
            100,
            0.50f,
            0.55f,
            0.08f,
            2.0,
            5.0,
            "炼制中",
            List.of(
                new AlchemySessionStore.StageHint(18, 5, "ci_she_hao×2", false, false),
                new AlchemySessionStore.StageHint(40, 8, "dan_sha×3", false, false)
            ),
            List.of("§7AdjustTemp(0.50)")
        );
    }

    private static AlchemySessionStore.Snapshot activeSessionAfterFeedAndIntervention() {
        return new AlchemySessionStore.Snapshot(
            "active_contract_recipe",
            true,
            46,
            100,
            0.57f,
            0.55f,
            0.08f,
            3.5,
            5.0,
            "炼制中",
            List.of(
                new AlchemySessionStore.StageHint(18, 5, "ci_she_hao×2", true, false),
                new AlchemySessionStore.StageHint(40, 8, "dan_sha×3", false, false)
            ),
            List.of("§7FeedStage(0)", "§7InjectQi(3.5)")
        );
    }
}
