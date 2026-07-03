package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import org.junit.jupiter.api.Test;

import java.util.Optional;
import java.util.OptionalInt;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;

/**
 * plan-scroll-reading-v1 P2 — {@link ScrollOpenGlowPlayer} 的 registry-free 守卫分支。
 *
 * <p>照抄 {@code FurnitureAuraVfxPlayerTest} 既有约定：{@code play(null, ...)} 必须安全
 * 短路返回（{@code GameplayVfxUtil.world(null)} → null → 早退），不触发真实粒子渲染管线
 * （headless 单测环境无 {@code MinecraftClient}）。真实粒子外观由 e2e/真游戏验收。
 */
class ScrollOpenGlowPlayerTest {

    private static VfxEventPayload.SpawnParticle payload(
        OptionalInt colorRgb,
        Optional<Double> strength,
        OptionalInt count
    ) {
        return new VfxEventPayload.SpawnParticle(
            ScrollOpenGlowPlayer.EVENT_ID,
            new double[] { 0.0, 64.0, 0.0 },
            Optional.empty(),
            colorRgb,
            strength,
            count,
            OptionalInt.empty()
        );
    }

    @Test
    void playReturnsWithoutMinecraftClient() {
        VfxEventPayload.SpawnParticle emptyPayload =
            payload(OptionalInt.empty(), Optional.empty(), OptionalInt.empty());

        assertDoesNotThrow(() -> new ScrollOpenGlowPlayer().play(null, emptyPayload));
    }

    @Test
    void playReturnsWithoutMinecraftClient_evenWithFullPayload() {
        // 带完整 payload（自定义颜色/强度/count）同样不应在无 MinecraftClient 时抛异常——
        // 早退发生在 GameplayVfxUtil.world(null) 之后，不依赖 payload 内容。
        VfxEventPayload.SpawnParticle fullPayload =
            payload(OptionalInt.of(0x123456), Optional.of(0.5), OptionalInt.of(20));

        assertDoesNotThrow(() -> new ScrollOpenGlowPlayer().play(null, fullPayload));
    }
}
