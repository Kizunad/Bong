package com.bong.client.visual.realm_vision;

import com.bong.client.hud.HudRenderCommand;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class PerceptionEdgeRendererTest {
    @Test
    void rendersWithPerDirectionLimit() {
        List<HudRenderCommand> out = new ArrayList<>();
        List<EdgeIndicatorCmd> indicators = new ArrayList<>();
        int x = 10;
        for (SenseKind kind : SenseKind.values()) {
            indicators.add(new EdgeIndicatorCmd(x++, 10, kind, 0.8, true, DirectionBucket.TOP));
        }
        PerceptionEdgeRenderer.append(out, indicators);
        assertEquals(3, out.size());
        assertTrue(out.stream().allMatch(HudRenderCommand::isEdgeIndicator));
    }

    @Test
    void colorAlphaScalesWithIntensity() {
        int low = PerceptionEdgeRenderer.colorFor(SenseKind.LIVING_QI, 0.0) >>> 24;
        int high = PerceptionEdgeRenderer.colorFor(SenseKind.LIVING_QI, 1.0) >>> 24;
        assertTrue(high > low);
    }

    @Test
    void spiritEyeWireKindMapsToPrivateHudMarker() {
        assertEquals(SenseKind.SPIRIT_EYE, SenseKind.fromWire("SpiritEye"));
        int color = PerceptionEdgeRenderer.colorFor(SenseKind.SPIRIT_EYE, 1.0);
        assertEquals(0x70FFD6, color & 0x00FFFFFF);
    }

    @Test
    void nicheIntrusionWireKindMapsToInkTraceMarker() {
        assertEquals(SenseKind.NICHE_INTRUSION_TRACE, SenseKind.fromWire("NicheIntrusionTrace"));
        int color = PerceptionEdgeRenderer.colorFor(SenseKind.NICHE_INTRUSION_TRACE, 1.0);
        assertEquals(0x4B4654, color & 0x00FFFFFF);
    }

    @Test
    void skipsInFovIndicatorsByContract() {
        List<HudRenderCommand> out = new ArrayList<>();
        PerceptionEdgeRenderer.append(out, List.of(
            new EdgeIndicatorCmd(160, 90, SenseKind.LIVING_QI, 0.8, false, DirectionBucket.TOP)
        ));
        assertEquals(0, out.size());
    }

    // plan-fauna-mimic-spider-v1 P2
    @Test
    void disguisedSpiderWireKindMapsToOrangeMarker() {
        assertEquals(
            SenseKind.DISGUISED_SPIDER,
            SenseKind.fromWire("DisguisedSpider"),
            "wire name DisguisedSpider 应映射到 DISGUISED_SPIDER"
        );
        int color = PerceptionEdgeRenderer.colorFor(SenseKind.DISGUISED_SPIDER, 1.0);
        assertEquals(
            0xFF8040,
            color & 0x00FFFFFF,
            "DISGUISED_SPIDER 颜色应为橙色 #FF8040（plan §P2 规格）"
        );
    }

    @Test
    void disguisedSpiderColorIntensityScales() {
        int lowAlpha  = PerceptionEdgeRenderer.colorFor(SenseKind.DISGUISED_SPIDER, 0.0) >>> 24;
        int highAlpha = PerceptionEdgeRenderer.colorFor(SenseKind.DISGUISED_SPIDER, 1.0) >>> 24;
        assertTrue(
            highAlpha > lowAlpha,
            "DISGUISED_SPIDER alpha 应随 intensity 增大（低 " + lowAlpha + " < 高 " + highAlpha + "）"
        );
    }

    @Test
    void all_sense_kinds_have_color_entry() {
        // 每个 SenseKind 都必须有颜色映射，添加新 variant 时此测试撞红提示补颜色
        for (SenseKind kind : SenseKind.values()) {
            int color = PerceptionEdgeRenderer.colorFor(kind, 0.5);
            assertTrue(
                (color & 0x00FFFFFF) != 0,
                "SenseKind." + kind + " 颜色不应为纯黑（可能 switch 缺少该 case）"
            );
        }
    }
}
