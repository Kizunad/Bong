package com.bong.client;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class CultivationScreenTest {
    @BeforeEach
    void setUp() {
        PlayerStateState.clear();
    }

    @AfterEach
    void tearDown() {
        PlayerStateState.clear();
    }

    @Test
    public void cultivationScreenContentIncludesRequiredReadOnlySections() {
        PlayerStateViewModel viewModel = PlayerStateViewModel.from(PlayerStateState.snapshotOf(
                new BongServerPayload.PlayerState("qi_refining_3", 78.0d, 100.0d, -0.2d, 0.35d, "blood_valley"),
                9_000L
        ));

        List<String> lines = CultivationScreen.contentLines(viewModel);

        assertTrue(lines.stream().anyMatch(line -> line.startsWith("境界: ")));
        assertTrue(lines.stream().anyMatch(line -> line.startsWith("真元: ")));
        assertTrue(lines.stream().anyMatch(line -> line.startsWith("因果 (karma): ")));
        assertTrue(lines.stream().anyMatch(line -> line.startsWith("综合实力: ")));
        assertTrue(lines.stream().anyMatch(line -> line.startsWith("├ 战斗: ")));
        assertTrue(lines.stream().anyMatch(line -> line.startsWith("├ 财富: ")));
        assertTrue(lines.stream().anyMatch(line -> line.startsWith("├ 社交: ")));
        assertTrue(lines.stream().anyMatch(line -> line.startsWith("└ 领地: ")));
        assertTrue(lines.stream().anyMatch(line -> line.startsWith("当前区域: ")));
        assertTrue(lines.stream().anyMatch(line -> line.equals("动态 XML UI: OFF")));
        assertTrue(lines.stream().anyMatch(line -> line.equals("界面模式: 只读本地状态")));
    }

    @Test
    public void dynamicXmlUiRemainsDisabledAndUnknownPayloadsAreIgnored() {
        assertFalse(CultivationUiFeatures.isDynamicXmlUiEnabled());
        assertTrue(CultivationUiFeatures.shouldIgnoreServerDrivenUiPayload("unknown_ui_payload"));
        assertTrue(CultivationUiFeatures.shouldIgnoreServerDrivenUiPayload("dynamic_xml_ui"));
    }

    @Test
    public void cultivationScreenUsesFallbackContentBeforeAnyPayloadArrives() {
        List<String> lines = CultivationScreen.contentLines(PlayerStateViewModel.empty());

        assertTrue(lines.stream().anyMatch(line -> line.equals("状态: 尚未收到 player_state 载荷")));
        assertTrue(lines.stream().anyMatch(line -> line.equals("当前区域: 未知区域")));
        assertTrue(lines.stream().anyMatch(line -> line.equals("动态 XML UI: OFF")));
    }

    @Test
    public void keypressGateOnlyOpensScreenWhenTriggeredAndNotAlreadyOpen() {
        assertTrue(BongClient.shouldOpenCultivationScreen(true, null));
        assertFalse(BongClient.shouldOpenCultivationScreen(false, null));
        assertFalse(BongClient.shouldOpenCultivationScreen(true, new CultivationScreen(PlayerStateViewModel.empty())));
    }
}
