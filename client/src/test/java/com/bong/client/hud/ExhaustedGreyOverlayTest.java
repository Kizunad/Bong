package com.bong.client.hud;

import com.bong.client.combat.store.StatusEffectStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * 虚脱灰幕现读标准 debuff 快照（{@link StatusEffectStore}，id "exhausted"）。
 * status_snapshot 全量替换 → /reset/到期时虚脱条目消失 → 灰幕自动隐藏（根治脱钩）。
 */
class ExhaustedGreyOverlayTest {
    @AfterEach void tearDown() {
        StatusEffectStore.resetForTests();
    }

    private static StatusEffectStore.Effect exhausted(long remainingMs) {
        return new StatusEffectStore.Effect(
            "exhausted", "虚脱", StatusEffectStore.Kind.DEBUFF, 1, remainingMs, 0xFFFF8030, "全力一击", 5);
    }

    @Test void hiddenWhenNoExhaustedEffect() {
        // 空快照（无虚脱）→ 不渲染。
        assertTrue(ExhaustedGreyOverlay.buildCommands(800, 600, 1_000L).isEmpty());
    }

    @Test void hiddenWhenExhaustedExpired() {
        // remaining_ms=0（status_snapshot 到期前下发的最后一帧理论上不含它，双重保险）→ 不渲染。
        StatusEffectStore.replace(List.of(exhausted(0L)));
        assertTrue(ExhaustedGreyOverlay.buildCommands(800, 600, 1_000L).isEmpty());
    }

    @Test void hiddenAfterSnapshotClearsExhausted() {
        // 先有虚脱 → 再来一帧空快照（模拟 /reset 或到期的 status_snapshot 全量替换）→ 灰幕消失。
        StatusEffectStore.replace(List.of(exhausted(10_000L)));
        assertFalse(ExhaustedGreyOverlay.buildCommands(800, 600, 1_000L).isEmpty());
        StatusEffectStore.replace(List.of());
        assertTrue(
            ExhaustedGreyOverlay.buildCommands(800, 600, 1_000L).isEmpty(),
            "空 status_snapshot 替换后虚脱灰幕必须消失（reset/到期同步清，无脱钩）");
    }

    @Test void drawsVignetteBarAndTextWhileExhausted() {
        StatusEffectStore.replace(List.of(exhausted(10_000L)));

        List<HudRenderCommand> commands = ExhaustedGreyOverlay.buildCommands(800, 600, 1_000L);

        assertEquals(4, commands.size());
        assertTrue(commands.get(0).isEdgeVignette());
        assertEquals(HudRenderLayer.VISUAL, commands.get(0).layer());
        assertTrue(commands.get(1).isRect());
        assertEquals(ExhaustedGreyOverlay.BAR_WIDTH, commands.get(1).width());
        assertTrue(commands.get(2).isRect());
        assertTrue(commands.get(3).isText());
        assertEquals("虚脱 10s", commands.get(3).text());
    }
}
