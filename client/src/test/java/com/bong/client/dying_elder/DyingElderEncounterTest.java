package com.bong.client.dying_elder;

import com.bong.client.hud.DyingElderHudPlanner;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.visual.realm_vision.SenseKind;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-dying-elder-v1 P3 — 垂死大能遭遇 Handler / Store / HudPlanner 饱和单测。
 *
 * <p>覆盖范围：
 * <ul>
 *   <li>{@link DyingElderEncounterStore}：activate / update / clear / clearOnDisconnect / betray / qiFraction / spiritEye</li>
 *   <li>{@link DyingElderEncounterHandler}：happy path appeared / dan_received / betrayal / dead_natural / dead_player_kill / invalid JSON</li>
 *   <li>{@link DyingElderHudPlanner}：active 布局 / inactive 空列表 / spiritEye on/off / betray 颜色 / qi bar 宽度 / 三按钮</li>
 *   <li>Channel 常量 pin：namespace=bong, path=elder_encounter</li>
 *   <li>HudRenderLayer.DYING_ELDER 存在</li>
 *   <li>守恒红线：qiFraction 超范围被截断</li>
 * </ul>
 */
public class DyingElderEncounterTest {

    @BeforeEach
    void reset() {
        DyingElderEncounterStore.resetForTest();
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Channel 常量 pin 测试
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void channelNamespaceIsBong() {
        assertEquals(
            "bong",
            DyingElderEncounterHandler.CHANNEL_NAMESPACE,
            "expected CHANNEL_NAMESPACE=bong because Bong mod uses bong: prefix for all custom channels, actual: "
                + DyingElderEncounterHandler.CHANNEL_NAMESPACE
        );
    }

    @Test
    void channelPathIsElderEncounter() {
        assertEquals(
            "elder_encounter",
            DyingElderEncounterHandler.CHANNEL_PATH,
            "expected CHANNEL_PATH=elder_encounter to match server RedisOutbound CH_ELDER_ENCOUNTER, actual: "
                + DyingElderEncounterHandler.CHANNEL_PATH
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // HudRenderLayer.DYING_ELDER 存在
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void dyingElderLayerExists() {
        // 枚举中必须存在 DYING_ELDER variant，否则 HUD 指令无法渲染
        HudRenderLayer layer = HudRenderLayer.DYING_ELDER;
        assertNotNull(
            layer,
            "expected HudRenderLayer.DYING_ELDER to exist because P3 registers a new HUD layer for elder encounter, actual: null"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // DyingElderEncounterStore — 初始状态
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void initialStateIsInactive() {
        assertFalse(
            DyingElderEncounterStore.isActive(),
            "expected isActive()=false on fresh store because no elder encounter has started, actual: true"
        );
    }

    @Test
    void initialBetrayProbabilityIsNull() {
        assertNull(
            DyingElderEncounterStore.getBetrayProbability(),
            "expected getBetrayProbability()=null initially because no server payload received yet, actual: "
                + DyingElderEncounterStore.getBetrayProbability()
        );
    }

    @Test
    void initialQiFractionIsZero() {
        assertEquals(
            0.0f,
            DyingElderEncounterStore.getQiFraction(),
            1e-5f,
            "expected getQiFraction()=0 initially because no qi data received yet, actual: "
                + DyingElderEncounterStore.getQiFraction()
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // DyingElderEncounterStore — activate
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void activateSetsActiveTrue() {
        DyingElderEncounterStore.activate("坍缩渊", 7, 1000L);

        assertTrue(
            DyingElderEncounterStore.isActive(),
            "expected isActive()=true after activate because appeared event starts the encounter, actual: false"
        );
    }

    @Test
    void activateSetsZoneDisplayName() {
        DyingElderEncounterStore.activate("青云峰", 3, 500L);

        assertEquals(
            "青云峰",
            DyingElderEncounterStore.getZoneDisplayName(),
            "expected zoneDisplayName=青云峰 after activate because zone is provided by the server payload, actual: "
                + DyingElderEncounterStore.getZoneDisplayName()
        );
    }

    @Test
    void activateSetsElderEntityIdx() {
        DyingElderEncounterStore.activate("坍缩渊", 42, 100L);

        assertEquals(
            42,
            DyingElderEncounterStore.getElderEntityIdx(),
            "expected elderEntityIdx=42 after activate, actual: " + DyingElderEncounterStore.getElderEntityIdx()
        );
    }

    @Test
    void activateSetsEventKindToAppeared() {
        DyingElderEncounterStore.activate("坍缩渊", 1, 0L);

        assertEquals(
            "appeared",
            DyingElderEncounterStore.getEventKind(),
            "expected eventKind=appeared after activate because initial event is always appeared, actual: "
                + DyingElderEncounterStore.getEventKind()
        );
    }

    @Test
    void activateSetsReceivedTick() {
        DyingElderEncounterStore.activate("坍缩渊", 1, 9999L);

        assertEquals(
            9999L,
            DyingElderEncounterStore.getReceivedTick(),
            "expected receivedTick=9999 after activate, actual: " + DyingElderEncounterStore.getReceivedTick()
        );
    }

    @Test
    void activateWithNullZoneUsesEmpty() {
        DyingElderEncounterStore.activate(null, 1, 0L);

        assertEquals(
            "",
            DyingElderEncounterStore.getZoneDisplayName(),
            "expected zoneDisplayName=empty string when null zone provided (defensive), actual: "
                + DyingElderEncounterStore.getZoneDisplayName()
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // DyingElderEncounterStore — update (dan_received)
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void updateDanReceivedKeepsActive() {
        DyingElderEncounterStore.activate("坍缩渊", 1, 0L);
        DyingElderEncounterStore.update("dan_received", 100L);

        assertTrue(
            DyingElderEncounterStore.isActive(),
            "expected isActive()=true after dan_received because non-death event should not clear the encounter, actual: false"
        );
    }

    @Test
    void updateDanReceivedSetsEventKind() {
        DyingElderEncounterStore.activate("坍缩渊", 1, 0L);
        DyingElderEncounterStore.update("dan_received", 100L);

        assertEquals(
            "dan_received",
            DyingElderEncounterStore.getEventKind(),
            "expected eventKind=dan_received after update, actual: "
                + DyingElderEncounterStore.getEventKind()
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // DyingElderEncounterStore — update (death kinds)
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void updateBetrayalClearsActive() {
        DyingElderEncounterStore.activate("坍缩渊", 1, 0L);
        DyingElderEncounterStore.update("betrayal", 200L);

        assertFalse(
            DyingElderEncounterStore.isActive(),
            "expected isActive()=false after betrayal because death event should end the encounter, actual: true"
        );
    }

    @Test
    void updateDeadNaturalClearsActive() {
        DyingElderEncounterStore.activate("坍缩渊", 1, 0L);
        DyingElderEncounterStore.update("dead_natural", 300L);

        assertFalse(
            DyingElderEncounterStore.isActive(),
            "expected isActive()=false after dead_natural because death ends the encounter, actual: true"
        );
    }

    @Test
    void updateDeadPlayerKillClearsActive() {
        DyingElderEncounterStore.activate("坍缩渊", 1, 0L);
        DyingElderEncounterStore.update("dead_player_kill", 400L);

        assertFalse(
            DyingElderEncounterStore.isActive(),
            "expected isActive()=false after dead_player_kill because death ends the encounter, actual: true"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // DyingElderEncounterStore — setBetrayProbability
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void setBetrayProbabilityStoresValue() {
        DyingElderEncounterStore.setBetrayProbability(0.75);

        assertEquals(
            0.75,
            DyingElderEncounterStore.getBetrayProbability(),
            1e-9,
            "expected getBetrayProbability()=0.75 after setBetrayProbability(0.75), actual: "
                + DyingElderEncounterStore.getBetrayProbability()
        );
    }

    @Test
    void setBetrayProbabilityNullAllowed() {
        DyingElderEncounterStore.setBetrayProbability(0.5);
        DyingElderEncounterStore.setBetrayProbability(null);

        assertNull(
            DyingElderEncounterStore.getBetrayProbability(),
            "expected getBetrayProbability()=null after reset to null, actual: "
                + DyingElderEncounterStore.getBetrayProbability()
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // DyingElderEncounterStore — qiFraction 守恒红线（截断）
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void qiFractionClampedToZeroOne() {
        DyingElderEncounterStore.setQiFraction(1.5f);
        assertEquals(
            1.0f,
            DyingElderEncounterStore.getQiFraction(),
            1e-5f,
            "expected qiFraction clamped to 1.0 when set to 1.5 because fraction must be in [0,1] to avoid bar overflow, actual: "
                + DyingElderEncounterStore.getQiFraction()
        );

        DyingElderEncounterStore.setQiFraction(-0.5f);
        assertEquals(
            0.0f,
            DyingElderEncounterStore.getQiFraction(),
            1e-5f,
            "expected qiFraction clamped to 0.0 when set to -0.5 because negative fraction is not valid, actual: "
                + DyingElderEncounterStore.getQiFraction()
        );
    }

    @Test
    void qiFractionHappyPath() {
        DyingElderEncounterStore.setQiFraction(0.6f);
        assertEquals(
            0.6f,
            DyingElderEncounterStore.getQiFraction(),
            1e-5f,
            "expected qiFraction=0.6 when set to 0.6, actual: " + DyingElderEncounterStore.getQiFraction()
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // DyingElderEncounterStore — clearOnDisconnect
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void clearOnDisconnectResetsAllFields() {
        DyingElderEncounterStore.activate("坍缩渊", 5, 100L);
        DyingElderEncounterStore.setBetrayProbability(0.8);
        DyingElderEncounterStore.setQiFraction(0.7f);
        DyingElderEncounterStore.setSpiritEyeActive(true);

        DyingElderEncounterStore.clearOnDisconnect();

        assertFalse(
            DyingElderEncounterStore.isActive(),
            "expected isActive()=false after clearOnDisconnect because elder encounter must not carry over across sessions, actual: true"
        );
        assertNull(
            DyingElderEncounterStore.getBetrayProbability(),
            "expected getBetrayProbability()=null after clearOnDisconnect because betray data belongs to the session, actual: "
                + DyingElderEncounterStore.getBetrayProbability()
        );
        assertEquals(
            0.0f,
            DyingElderEncounterStore.getQiFraction(),
            1e-5f,
            "expected qiFraction=0.0 after clearOnDisconnect, actual: " + DyingElderEncounterStore.getQiFraction()
        );
        assertFalse(
            DyingElderEncounterStore.isSpiritEyeActive(),
            "expected spiritEyeActive=false after clearOnDisconnect, actual: true"
        );
        assertEquals(
            0L,
            DyingElderEncounterStore.getReceivedTick(),
            "expected receivedTick=0 after clearOnDisconnect, actual: " + DyingElderEncounterStore.getReceivedTick()
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // DyingElderEncounterHandler — happy path: appeared
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void handlerAppearedActivatesStore() {
        String payload = "{\"zone_name\":\"坍缩渊\",\"elder_entity_idx\":3,\"event_kind\":\"appeared\","
            + "\"betray_probability\":0.45,\"dan_count\":0,\"offered_skill_id\":\"skill_void_step\",\"server_tick\":500}";

        boolean result = DyingElderEncounterHandler.handle(payload);

        assertTrue(result, "expected handle to return true on valid appeared payload");
        assertTrue(
            DyingElderEncounterStore.isActive(),
            "expected isActive()=true after handler processes appeared payload, actual: false"
        );
        assertEquals(
            "坍缩渊",
            DyingElderEncounterStore.getZoneDisplayName(),
            "expected zoneDisplayName=坍缩渊 from appeared payload, actual: "
                + DyingElderEncounterStore.getZoneDisplayName()
        );
        assertEquals(
            0.45,
            DyingElderEncounterStore.getBetrayProbability(),
            1e-6,
            "expected betrayProbability=0.45 from appeared payload, actual: "
                + DyingElderEncounterStore.getBetrayProbability()
        );
    }

    @Test
    void handlerAppearedSetsElderEntityIdx() {
        String payload = "{\"zone_name\":\"青云峰\",\"elder_entity_idx\":17,\"event_kind\":\"appeared\","
            + "\"betray_probability\":0.3,\"dan_count\":0,\"offered_skill_id\":\"s\",\"server_tick\":1}";
        DyingElderEncounterHandler.handle(payload);

        assertEquals(
            17,
            DyingElderEncounterStore.getElderEntityIdx(),
            "expected elderEntityIdx=17 from appeared payload, actual: "
                + DyingElderEncounterStore.getElderEntityIdx()
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // DyingElderEncounterHandler — dan_received
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void handlerDanReceivedUpdatesEventKind() {
        // First activate
        DyingElderEncounterHandler.handle("{\"zone_name\":\"z\",\"elder_entity_idx\":1,\"event_kind\":\"appeared\","
            + "\"betray_probability\":0.3,\"dan_count\":0,\"offered_skill_id\":\"s\",\"server_tick\":1}");
        // Then update
        boolean result = DyingElderEncounterHandler.handle("{\"zone_name\":\"z\",\"elder_entity_idx\":1,\"event_kind\":\"dan_received\","
            + "\"betray_probability\":0.35,\"dan_count\":2,\"offered_skill_id\":\"s\",\"server_tick\":200}");

        assertTrue(result, "expected handle to return true on valid dan_received payload");
        assertEquals(
            "dan_received",
            DyingElderEncounterStore.getEventKind(),
            "expected eventKind=dan_received after handler processes dan_received payload, actual: "
                + DyingElderEncounterStore.getEventKind()
        );
        assertTrue(
            DyingElderEncounterStore.isActive(),
            "expected isActive()=true after dan_received because encounter continues, actual: false"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // DyingElderEncounterHandler — death events
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void handlerBetrayalClearsActive() {
        DyingElderEncounterHandler.handle("{\"zone_name\":\"z\",\"elder_entity_idx\":1,\"event_kind\":\"appeared\","
            + "\"betray_probability\":0.9,\"dan_count\":0,\"offered_skill_id\":\"s\",\"server_tick\":1}");
        boolean result = DyingElderEncounterHandler.handle("{\"zone_name\":\"z\",\"elder_entity_idx\":1,\"event_kind\":\"betrayal\","
            + "\"betray_probability\":0.9,\"dan_count\":3,\"offered_skill_id\":\"s\",\"server_tick\":300}");

        assertTrue(result, "expected handle to return true on valid betrayal payload");
        assertFalse(
            DyingElderEncounterStore.isActive(),
            "expected isActive()=false after betrayal payload because the encounter ends, actual: true"
        );
        assertEquals(
            "betrayal",
            DyingElderEncounterStore.getEventKind(),
            "expected eventKind=betrayal after betrayal payload, actual: "
                + DyingElderEncounterStore.getEventKind()
        );
    }

    @Test
    void handlerDeadNaturalClearsActive() {
        DyingElderEncounterHandler.handle("{\"zone_name\":\"z\",\"elder_entity_idx\":1,\"event_kind\":\"appeared\","
            + "\"betray_probability\":0.4,\"dan_count\":0,\"offered_skill_id\":\"s\",\"server_tick\":1}");
        DyingElderEncounterHandler.handle("{\"zone_name\":\"z\",\"elder_entity_idx\":1,\"event_kind\":\"dead_natural\","
            + "\"betray_probability\":0.4,\"dan_count\":5,\"offered_skill_id\":\"s\",\"server_tick\":400}");

        assertFalse(
            DyingElderEncounterStore.isActive(),
            "expected isActive()=false after dead_natural payload because the encounter ends, actual: true"
        );
    }

    @Test
    void handlerDeadPlayerKillClearsActive() {
        DyingElderEncounterHandler.handle("{\"zone_name\":\"z\",\"elder_entity_idx\":1,\"event_kind\":\"appeared\","
            + "\"betray_probability\":0.4,\"dan_count\":0,\"offered_skill_id\":\"s\",\"server_tick\":1}");
        DyingElderEncounterHandler.handle("{\"zone_name\":\"z\",\"elder_entity_idx\":1,\"event_kind\":\"dead_player_kill\","
            + "\"betray_probability\":0.4,\"dan_count\":5,\"offered_skill_id\":\"s\",\"server_tick\":400}");

        assertFalse(
            DyingElderEncounterStore.isActive(),
            "expected isActive()=false after dead_player_kill payload because the encounter ends, actual: true"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // DyingElderEncounterHandler — 边界 / 错误路径
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void handlerReturnsFalseOnInvalidJson() {
        boolean result = DyingElderEncounterHandler.handle("not-valid-json");
        assertFalse(
            result,
            "expected handle to return false on invalid JSON payload because handler must not throw, actual: true"
        );
    }

    @Test
    void handlerDoesNotThrowOnEmptyString() {
        assertDoesNotThrow(
            () -> DyingElderEncounterHandler.handle(""),
            "expected handle to NOT throw on empty string because the elder_encounter channel must be resilient"
        );
    }

    @Test
    void handlerReturnsFalseOnEmptyString() {
        boolean result = DyingElderEncounterHandler.handle("");
        assertFalse(
            result,
            "expected handle to return false on empty input, actual: true"
        );
    }

    @Test
    void handlerHandlesMissingOptionalFields() {
        // Minimal payload — only event_kind=appeared, everything else missing
        String payload = "{\"event_kind\":\"appeared\"}";
        boolean result = DyingElderEncounterHandler.handle(payload);

        assertTrue(
            result,
            "expected handle to return true even with missing optional fields, actual: false"
        );
        assertTrue(
            DyingElderEncounterStore.isActive(),
            "expected isActive()=true after minimal appeared payload, actual: false"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // DyingElderHudPlanner — inactive 时返回空列表
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void plannerReturnsEmptyWhenInactive() {
        List<HudRenderCommand> cmds = DyingElderHudPlanner.buildCommands(
            false, 0.5f, false, null, "坍缩渊", 800, 600
        );

        assertTrue(
            cmds.isEmpty(),
            "expected empty command list when inactive because DYING_ELDER HUD must not appear without an encounter, actual size: "
                + cmds.size()
        );
    }

    @Test
    void plannerReturnsEmptyWithZeroScreenSize() {
        List<HudRenderCommand> cmds = DyingElderHudPlanner.buildCommands(
            true, 0.5f, false, null, "坍缩渊", 0, 0
        );

        assertTrue(
            cmds.isEmpty(),
            "expected empty command list with zero screen size because HUD cannot render without dimensions, actual size: "
                + cmds.size()
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // DyingElderHudPlanner — active 时渲染指令
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void plannerReturnsNonEmptyWhenActive() {
        List<HudRenderCommand> cmds = DyingElderHudPlanner.buildCommands(
            true, 0.5f, false, null, "坍缩渊", 800, 600
        );

        assertFalse(
            cmds.isEmpty(),
            "expected non-empty command list when active because HUD must render during an encounter, actual: empty"
        );
    }

    @Test
    void plannerAllCommandsUseDyingElderLayer() {
        List<HudRenderCommand> cmds = DyingElderHudPlanner.buildCommands(
            true, 0.5f, true, 0.7, "坍缩渊", 800, 600
        );

        for (HudRenderCommand cmd : cmds) {
            assertEquals(
                HudRenderLayer.DYING_ELDER,
                cmd.layer(),
                "expected all planner commands to use HudRenderLayer.DYING_ELDER because this planner owns that layer exclusively, actual: "
                    + cmd.layer()
            );
        }
    }

    @Test
    void plannerContainsThreeButtonRects() {
        List<HudRenderCommand> cmds = DyingElderHudPlanner.buildCommands(
            true, 0.5f, false, null, "坍缩渊", 800, 600
        );

        // 3 buttons × 2 commands each (rect + text) = 6 button-related commands
        // count rect commands with BTN_HEIGHT height
        long btnRects = cmds.stream()
            .filter(c -> c.kind() == HudRenderCommand.Kind.RECT)
            .filter(c -> c.height() == DyingElderHudPlanner.BTN_HEIGHT)
            .count();

        assertEquals(
            3,
            btnRects,
            "expected exactly 3 button rect commands (give/refuse/delay) in the planner output, actual: " + btnRects
        );
    }

    @Test
    void plannerQiBarWidthScalesWithFraction() {
        List<HudRenderCommand> cmds100 = DyingElderHudPlanner.buildCommands(
            true, 1.0f, false, null, "坍缩渊", 800, 600
        );
        List<HudRenderCommand> cmds50 = DyingElderHudPlanner.buildCommands(
            true, 0.5f, false, null, "坍缩渊", 800, 600
        );
        List<HudRenderCommand> cmds0 = DyingElderHudPlanner.buildCommands(
            true, 0.0f, false, null, "坍缩渊", 800, 600
        );

        // The qi bar foreground is the rect with COLOR_QI_BAR_FG
        int maxBarWidth = DyingElderHudPlanner.PANEL_WIDTH - DyingElderHudPlanner.QI_BAR_PADDING * 2;

        // At qi=1.0, the FG rect should have full width
        int fgWidth100 = cmds100.stream()
            .filter(c -> c.kind() == HudRenderCommand.Kind.RECT && c.color() == DyingElderHudPlanner.COLOR_QI_BAR_FG)
            .mapToInt(HudRenderCommand::width)
            .sum();
        assertEquals(
            maxBarWidth,
            fgWidth100,
            "expected qi bar full width at qiFraction=1.0 because bar should fill completely, actual: " + fgWidth100
        );

        // At qi=0.0, no FG rect should appear
        long fgRects0 = cmds0.stream()
            .filter(c -> c.kind() == HudRenderCommand.Kind.RECT && c.color() == DyingElderHudPlanner.COLOR_QI_BAR_FG)
            .count();
        assertEquals(
            0,
            fgRects0,
            "expected no qi bar FG rect at qiFraction=0.0 because empty bar has no foreground, actual: " + fgRects0
        );

        // At qi=0.5, FG width should be roughly half of max
        int fgWidth50 = cmds50.stream()
            .filter(c -> c.kind() == HudRenderCommand.Kind.RECT && c.color() == DyingElderHudPlanner.COLOR_QI_BAR_FG)
            .mapToInt(HudRenderCommand::width)
            .sum();
        assertEquals(
            Math.round(maxBarWidth * 0.5f),
            fgWidth50,
            "expected qi bar half width at qiFraction=0.5 because bar is proportional, actual: " + fgWidth50
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // DyingElderHudPlanner — SpiritEye / 背叛概率 文字
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void plannerShowsGasHasAnomalyWhenSpiritEyeOff() {
        List<HudRenderCommand> cmds = DyingElderHudPlanner.buildCommands(
            true, 0.5f, false, 0.8, "坍缩渊", 800, 600
        );

        boolean hasAnomalyText = cmds.stream()
            .filter(c -> c.kind() == HudRenderCommand.Kind.SCALED_TEXT)
            .anyMatch(c -> c.text() != null && c.text().contains("气息有异"));

        assertTrue(
            hasAnomalyText,
            "expected '气息有异' text when spiritEyeActive=false because player cannot read betray probability without SpiritEye, actual: not found"
        );
    }

    @Test
    void plannerShowsBetrayPercentWhenSpiritEyeOn() {
        List<HudRenderCommand> cmds = DyingElderHudPlanner.buildCommands(
            true, 0.5f, true, 0.75, "坍缩渊", 800, 600
        );

        boolean hasBetrayPercent = cmds.stream()
            .filter(c -> c.kind() == HudRenderCommand.Kind.SCALED_TEXT)
            .anyMatch(c -> c.text() != null && c.text().contains("叛") && c.text().contains("%"));

        assertTrue(
            hasBetrayPercent,
            "expected betray percentage text when spiritEyeActive=true with known betrayProbability, actual: not found"
        );
    }

    @Test
    void plannerShowsGasHasAnomalyWhenBetrayProbabilityNull() {
        List<HudRenderCommand> cmds = DyingElderHudPlanner.buildCommands(
            true, 0.5f, true, null, "坍缩渊", 800, 600
        );

        boolean hasAnomalyText = cmds.stream()
            .filter(c -> c.kind() == HudRenderCommand.Kind.SCALED_TEXT)
            .anyMatch(c -> c.text() != null && c.text().contains("气息有异"));

        assertTrue(
            hasAnomalyText,
            "expected '气息有异' text when spiritEyeActive=true but betrayProbability=null because server hasn't provided data yet, actual: not found"
        );
    }

    @Test
    void plannerHighBetrayUsesRedColor() {
        // betray >= 0.6 should use COLOR_BETRAY_HIGH (red-ish)
        List<HudRenderCommand> cmds = DyingElderHudPlanner.buildCommands(
            true, 0.5f, true, 0.9, "坍缩渊", 800, 600
        );

        boolean hasHighColor = cmds.stream()
            .filter(c -> c.kind() == HudRenderCommand.Kind.SCALED_TEXT)
            .anyMatch(c -> c.color() == DyingElderHudPlanner.COLOR_BETRAY_HIGH);

        assertTrue(
            hasHighColor,
            "expected COLOR_BETRAY_HIGH for betray_probability=0.9 because high risk should be red to warn player, actual: not found"
        );
    }

    @Test
    void plannerLowBetrayUsesGreenColor() {
        // betray < 0.6 should use COLOR_BETRAY_LOW (green-ish)
        List<HudRenderCommand> cmds = DyingElderHudPlanner.buildCommands(
            true, 0.5f, true, 0.3, "坍缩渊", 800, 600
        );

        boolean hasLowColor = cmds.stream()
            .filter(c -> c.kind() == HudRenderCommand.Kind.SCALED_TEXT)
            .anyMatch(c -> c.color() == DyingElderHudPlanner.COLOR_BETRAY_LOW);

        assertTrue(
            hasLowColor,
            "expected COLOR_BETRAY_LOW for betray_probability=0.3 because low risk should be green, actual: not found"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // DyingElderHudPlanner — 布局常量 pin
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void panelWidthIs180() {
        assertEquals(
            180,
            DyingElderHudPlanner.PANEL_WIDTH,
            "expected PANEL_WIDTH=180 because plan P3 specifies this dimension for the elder encounter HUD, actual: "
                + DyingElderHudPlanner.PANEL_WIDTH
        );
    }

    @Test
    void threeButtonsLabelsExist() {
        List<HudRenderCommand> cmds = DyingElderHudPlanner.buildCommands(
            true, 0.5f, false, null, "坍缩渊", 800, 600
        );

        boolean hasGiveDan = cmds.stream()
            .filter(c -> c.kind() == HudRenderCommand.Kind.SCALED_TEXT)
            .anyMatch(c -> c.text() != null && c.text().contains("给丹"));
        boolean hasRefuse = cmds.stream()
            .filter(c -> c.kind() == HudRenderCommand.Kind.SCALED_TEXT)
            .anyMatch(c -> c.text() != null && c.text().contains("拒绝"));
        boolean hasDelay = cmds.stream()
            .filter(c -> c.kind() == HudRenderCommand.Kind.SCALED_TEXT)
            .anyMatch(c -> c.text() != null && c.text().contains("拖延"));

        assertTrue(hasGiveDan, "expected '给丹 [G]' button text in HUD commands because P3 spec lists 给丹 as first button");
        assertTrue(hasRefuse, "expected '拒绝 [H]' button text in HUD commands because P3 spec lists 拒绝 as second button");
        assertTrue(hasDelay, "expected '拖延 [J]' button text in HUD commands because P3 spec lists 拖延 as third button");
    }

    @Test
    void zoneTitleIncludedInHud() {
        List<HudRenderCommand> cmds = DyingElderHudPlanner.buildCommands(
            true, 0.5f, false, null, "青云峰", 800, 600
        );

        boolean hasTitleText = cmds.stream()
            .filter(c -> c.kind() == HudRenderCommand.Kind.SCALED_TEXT)
            .anyMatch(c -> c.text() != null && c.text().contains("青云峰"));

        assertTrue(
            hasTitleText,
            "expected zone name '青云峰' in HUD title text because the zone identifies the encounter location, actual: not found"
        );
    }

    @Test
    void emptyZoneUsesDefaultTitle() {
        List<HudRenderCommand> cmds = DyingElderHudPlanner.buildCommands(
            true, 0.5f, false, null, "", 800, 600
        );

        boolean hasDefaultTitle = cmds.stream()
            .filter(c -> c.kind() == HudRenderCommand.Kind.SCALED_TEXT)
            .anyMatch(c -> c.text() != null && c.text().contains("垂死大能"));

        assertTrue(
            hasDefaultTitle,
            "expected default title '垂死大能' when zone is empty, actual: not found"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // 状态转换测试
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void secondActivateOverridesFirst() {
        DyingElderEncounterStore.activate("坍缩渊", 1, 100L);
        DyingElderEncounterStore.activate("青云峰", 2, 200L);

        assertEquals(
            "青云峰",
            DyingElderEncounterStore.getZoneDisplayName(),
            "expected second activate to override zone name because encounters replace each other, actual: "
                + DyingElderEncounterStore.getZoneDisplayName()
        );
        assertEquals(
            2,
            DyingElderEncounterStore.getElderEntityIdx(),
            "expected second activate to override entity idx, actual: " + DyingElderEncounterStore.getElderEntityIdx()
        );
    }

    @Test
    void reactivateAfterDeathWorks() {
        DyingElderEncounterStore.activate("坍缩渊", 1, 100L);
        DyingElderEncounterStore.update("dead_natural", 200L);
        assertFalse(DyingElderEncounterStore.isActive());

        // New encounter starts
        DyingElderEncounterStore.activate("青云峰", 5, 300L);
        assertTrue(
            DyingElderEncounterStore.isActive(),
            "expected isActive()=true after re-activate following death because encounters can repeat, actual: false"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // B2 fix: BongHudOrchestrator 已集成 DyingElderHudPlanner
    // ──────────────────────────────────────────────────────────────────────────

    /**
     * B2 fix: 验证 BongHudOrchestrator.java 源码包含 DyingElderHudPlanner.buildCommands 调用。
     *
     * <p>BongHudOrchestrator 是纯静态工厂，无法在测试环境中执行（依赖 Minecraft 渲染上下文）。
     * 通过读取源码文本确认调用链存在，锁住"HUD planner 已被调用"这一契约。
     */
    @Test
    void bongHudOrchestratorInvokesDyingElderHudPlanner() throws Exception {
        // 定位 BongHudOrchestrator.java 源码（从 test class root 向上定位）
        java.nio.file.Path testClasses = java.nio.file.Path.of("").toAbsolutePath().normalize();
        // 解析 client module root
        java.nio.file.Path clientRoot;
        if (java.nio.file.Files.isDirectory(testClasses.resolve("src"))) {
            clientRoot = testClasses;
        } else if (java.nio.file.Files.isDirectory(testClasses.resolve("client").resolve("src"))) {
            clientRoot = testClasses.resolve("client");
        } else {
            // fallback: gradle test working dir is usually module root
            clientRoot = testClasses;
        }
        java.nio.file.Path orchestratorSrc = clientRoot.resolve(
            "src/main/java/com/bong/client/hud/BongHudOrchestrator.java"
        );
        assertTrue(
            java.nio.file.Files.exists(orchestratorSrc),
            "BongHudOrchestrator.java must exist at " + orchestratorSrc.toAbsolutePath()
                + " — cannot verify B2 fix without source file"
        );
        String src = java.nio.file.Files.readString(orchestratorSrc);
        assertTrue(
            src.contains("DyingElderHudPlanner.buildCommands"),
            "expected BongHudOrchestrator to call DyingElderHudPlanner.buildCommands because "
                + "without this call the dying elder HUD will never render (B2 blocker fix), actual: call not found in source"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // M1 fix: SpiritualSenseTargetsHandler 已集成 setSpiritEyeActive
    // ──────────────────────────────────────────────────────────────────────────

    /**
     * M1 fix: 验证 SpiritualSenseTargetsHandler.java 源码包含 setSpiritEyeActive 调用。
     *
     * <p>SpiritualSenseTargetsHandler 依赖 Minecraft 网络上下文，无法直接在测试环境执行。
     * 通过读取源码文本锁住"setSpiritEyeActive 已被调用"这一契约。
     */
    @Test
    void spiritualSenseTargetsHandlerCallsSetSpiritEyeActive() throws Exception {
        java.nio.file.Path testClasses = java.nio.file.Path.of("").toAbsolutePath().normalize();
        java.nio.file.Path clientRoot;
        if (java.nio.file.Files.isDirectory(testClasses.resolve("src"))) {
            clientRoot = testClasses;
        } else if (java.nio.file.Files.isDirectory(testClasses.resolve("client").resolve("src"))) {
            clientRoot = testClasses.resolve("client");
        } else {
            clientRoot = testClasses;
        }
        java.nio.file.Path handlerSrc = clientRoot.resolve(
            "src/main/java/com/bong/client/network/SpiritualSenseTargetsHandler.java"
        );
        assertTrue(
            java.nio.file.Files.exists(handlerSrc),
            "SpiritualSenseTargetsHandler.java must exist at " + handlerSrc.toAbsolutePath()
        );
        String src = java.nio.file.Files.readString(handlerSrc);
        assertTrue(
            src.contains("setSpiritEyeActive"),
            "expected SpiritualSenseTargetsHandler to call DyingElderEncounterStore.setSpiritEyeActive "
                + "because without this call the spirit eye state never updates and the dying elder HUD "
                + "will always show '气息有异' even when SpiritEye is active (M1 fix), actual: call not found in source"
        );
        assertTrue(
            src.contains("SPIRIT_EYE"),
            "expected SpiritualSenseTargetsHandler to check for SenseKind.SPIRIT_EYE entries "
                + "to derive spiritEyeActive state, actual: check not found in source"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // M1 fix: DyingElderEncounterStore.setSpiritEyeActive 正确更新 spiritEyeActive
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void setSpiritEyeActiveUpdatesState() {
        // 初始为 false
        assertFalse(
            DyingElderEncounterStore.isSpiritEyeActive(),
            "expected spiritEyeActive=false initially because no perception data received yet"
        );

        // 激活
        DyingElderEncounterStore.setSpiritEyeActive(true);
        assertTrue(
            DyingElderEncounterStore.isSpiritEyeActive(),
            "expected spiritEyeActive=true after setSpiritEyeActive(true), actual: false"
        );

        // 关闭
        DyingElderEncounterStore.setSpiritEyeActive(false);
        assertFalse(
            DyingElderEncounterStore.isSpiritEyeActive(),
            "expected spiritEyeActive=false after setSpiritEyeActive(false), actual: true"
        );
    }

    @Test
    void spiritEyeActiveTrueShowsBetrayProbabilityInHud() {
        // M1 fix 端到端: spiritEyeActive=true 时 HUD 显示背叛概率数字
        DyingElderEncounterStore.activate("坍缩渊", 1, 100L);
        DyingElderEncounterStore.setBetrayProbability(0.8);
        DyingElderEncounterStore.setSpiritEyeActive(true);

        List<HudRenderCommand> cmds = DyingElderHudPlanner.buildCommands(1024, 768);
        boolean hasBetrayPercent = cmds.stream()
            .filter(c -> c.kind() == HudRenderCommand.Kind.SCALED_TEXT)
            .anyMatch(c -> c.text() != null && c.text().contains("叛") && c.text().contains("%"));
        assertTrue(
            hasBetrayPercent,
            "expected betray percentage text in HUD when spiritEyeActive=true and encounter active "
                + "because SpiritEye allows reading the elder's true betrayal probability (M1 fix), actual: not found"
        );
    }

    @Test
    void spiritEyeActiveFalseShowsGasHasAnomalyInHud() {
        // M1 fix 端到端: spiritEyeActive=false 时 HUD 显示"气息有异"
        DyingElderEncounterStore.activate("坍缩渊", 1, 100L);
        DyingElderEncounterStore.setBetrayProbability(0.8);
        DyingElderEncounterStore.setSpiritEyeActive(false);

        List<HudRenderCommand> cmds = DyingElderHudPlanner.buildCommands(1024, 768);
        boolean hasAnomalyText = cmds.stream()
            .filter(c -> c.kind() == HudRenderCommand.Kind.SCALED_TEXT)
            .anyMatch(c -> c.text() != null && c.text().contains("气息有异"));
        assertTrue(
            hasAnomalyText,
            "expected '气息有异' in HUD when spiritEyeActive=false because without SpiritEye "
                + "the player cannot read the elder's betrayal probability (M1 fix), actual: not found"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // SenseKind.SPIRIT_EYE 存在（M1 fix 依赖）
    // ──────────────────────────────────────────────────────────────────────────

    @Test
    void senseKindSpiritEyeVariantExists() {
        // M1 fix: SenseKind.SPIRIT_EYE enum variant 必须存在，
        // SpiritualSenseTargetsHandler 依赖此 variant 判断 spiritEye 是否激活
        SenseKind spiritEye = SenseKind.SPIRIT_EYE;
        assertNotNull(
            spiritEye,
            "expected SenseKind.SPIRIT_EYE to exist because SpiritualSenseTargetsHandler checks this "
                + "to derive DyingElderEncounterStore.spiritEyeActive (M1 fix)"
        );
        assertEquals(
            SenseKind.SPIRIT_EYE,
            SenseKind.fromWire("SpiritEye"),
            "expected SenseKind.fromWire('SpiritEye') == SPIRIT_EYE because server sends 'SpiritEye' kind string"
        );
    }
}
