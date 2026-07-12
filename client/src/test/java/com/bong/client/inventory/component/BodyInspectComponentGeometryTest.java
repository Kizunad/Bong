package com.bong.client.inventory.component;

import com.bong.client.inventory.model.BodyPart;
import com.bong.client.inventory.model.MeridianChannel;
import com.bong.client.inventory.model.bodyplan.BodyPlanLayout;
import com.bong.client.inventory.model.bodyplan.MeridianPath;
import com.bong.client.inventory.model.bodyplan.PartAnchor;
import com.bong.client.inventory.model.bodyplan.Point2;
import com.bong.client.inventory.model.bodyplan.SilhouettePart;
import com.bong.client.inventory.state.BodyPlanLayoutStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-race-system-v1 P2b — {@code BodyInspectComponent} 几何改造像素级回归。
 *
 * <p>数值全部转录自 {@code server/assets/body_plans/layouts/humanoid.json}（该文件本身
 * 是 P2a 从本组件原硬编码表逐值抽取而来，见 commit 31bab673）。本测试反向验证：
 * store 已加载 humanoid layout 时的渲染坐标，与 store 为空时的硬编码 fallback
 * 坐标**逐值相等**——证明改造后 humanoid 现状（人形玩家最常见路径）零回归。
 */
class BodyInspectComponentGeometryTest {

    @AfterEach
    void tearDown() {
        BodyPlanLayoutStore.resetForTests();
    }

    private static BodyPlanLayout humanoidSubsetLayout() {
        List<SilhouettePart> silhouette = List.of(
            new SilhouettePart("head", List.of(
                new Point2(0.434524, 0.025424),
                new Point2(0.565476, 0.025424),
                new Point2(0.565476, 0.118644),
                new Point2(0.434524, 0.118644)
            )),
            new SilhouettePart("chest", List.of(
                new Point2(0.369048, 0.144068),
                new Point2(0.630952, 0.144068),
                new Point2(0.630952, 0.279661),
                new Point2(0.369048, 0.279661)
            ))
        );
        List<PartAnchor> anchors = List.of(
            new PartAnchor("head", new Point2(0.5, 0.042373)),
            new PartAnchor("chest", new Point2(0.5, 0.20339)),
            new PartAnchor("left_thigh", new Point2(0.369048, 0.559322)),
            new PartAnchor("right_foot", new Point2(0.613095, 0.805085))
        );
        List<MeridianPath> meridianPaths = List.of(
            new MeridianPath("lung", List.of(
                new Point2(0.452381, 0.169492),
                new Point2(0.392857, 0.211864),
                new Point2(0.333333, 0.313559),
                new Point2(0.297619, 0.423729),
                new Point2(0.285714, 0.474576)
            ))
        );
        return new BodyPlanLayout("humanoid", silhouette, anchors, meridianPaths, List.of());
    }

    // ── fallback path (store empty) reproduces the historical hardcoded table ──
    @Test
    void fallbackRectAndAnchorMatchHistoricalHardcodedTable() {
        assertArrayEquals(new int[]{-11, 6, 11, 28}, BodyInspectComponent.fallbackBodyPartRect(BodyPart.HEAD));
        assertArrayEquals(new int[]{-22, 34, 22, 66}, BodyInspectComponent.fallbackBodyPartRect(BodyPart.CHEST));
        assertArrayEquals(new int[]{0, 10}, BodyInspectComponent.fallbackBodyPartAnchor(BodyPart.HEAD));
        assertArrayEquals(new int[]{-22, 132}, BodyInspectComponent.fallbackBodyPartAnchor(BodyPart.LEFT_THIGH));
    }

    @Test
    void noLayoutLoadedUsesFallbackGeometryVerbatim() {
        BodyPlanLayoutStore.resetForTests(); // no current layout
        assertArrayEquals(BodyInspectComponent.fallbackBodyPartRect(BodyPart.HEAD),
            BodyInspectComponent.bodyPartRectForTests(BodyPart.HEAD));
        assertArrayEquals(BodyInspectComponent.fallbackBodyPartAnchor(BodyPart.RIGHT_FOOT),
            BodyInspectComponent.bodyPartAnchorForTests(BodyPart.RIGHT_FOOT));
    }

    // ── store-driven path (humanoid layout loaded) must equal the same historical values ──
    @Test
    void humanoidLayoutLoadedReproducesExactHardcodedGeometry_head() {
        BodyPlanLayoutStore.putLayout(humanoidSubsetLayout());
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");

        assertArrayEquals(new int[]{-11, 6, 11, 28}, BodyInspectComponent.bodyPartRectForTests(BodyPart.HEAD),
            "head silhouette bbox must round-trip to the exact historical rect");
        assertArrayEquals(new int[]{0, 10}, BodyInspectComponent.bodyPartAnchorForTests(BodyPart.HEAD),
            "head anchor must round-trip to the exact historical anchor");
    }

    @Test
    void humanoidLayoutLoadedReproducesExactHardcodedGeometry_chestAndLimbs() {
        BodyPlanLayoutStore.putLayout(humanoidSubsetLayout());
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");

        assertArrayEquals(new int[]{-22, 34, 22, 66}, BodyInspectComponent.bodyPartRectForTests(BodyPart.CHEST));
        assertArrayEquals(new int[]{-22, 132}, BodyInspectComponent.bodyPartAnchorForTests(BodyPart.LEFT_THIGH));
        assertArrayEquals(new int[]{19, 190}, BodyInspectComponent.bodyPartAnchorForTests(BodyPart.RIGHT_FOOT));
    }

    @Test
    void humanoidLayoutLoadedReproducesExactMeridianPath() {
        BodyPlanLayoutStore.putLayout(humanoidSubsetLayout());
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");

        int[][] expected = {
            {-8, 40}, {-18, 50}, {-28, 74}, {-34, 100}, {-36, 112}
        };
        int[][] actual = BodyInspectComponent.meridianPathPointsForTests(MeridianChannel.LU);
        org.junit.jupiter.api.Assertions.assertEquals(expected.length, actual.length);
        for (int i = 0; i < expected.length; i++) {
            assertArrayEquals(expected[i], actual[i], "LU waypoint " + i + " must round-trip exactly");
        }
        assertArrayEquals(new int[]{-36, 112}, BodyInspectComponent.meridianAnchorForTests(MeridianChannel.LU),
            "meridian tooltip anchor is the path's last waypoint");
    }

    // ── channel not declared by the loaded layout falls back to the historical table ──
    @Test
    void meridianChannelMissingFromLayoutFallsBackToHardcodedTable() {
        BodyPlanLayoutStore.putLayout(humanoidSubsetLayout()); // only declares "lung"
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");

        // HT ("heart") isn't in our subset layout — must fall back, not silently vanish.
        int[][] fallbackHt = BodyInspectComponent.meridianPathPointsForTests(MeridianChannel.HT);
        assertArrayEquals(new int[]{-16, 48}, fallbackHt[0]);
    }

    // ── part not declared by the loaded layout falls back to the historical table ──
    @Test
    void bodyPartMissingFromLayoutFallsBackToHardcodedTable() {
        BodyPlanLayoutStore.putLayout(humanoidSubsetLayout()); // only declares head/chest silhouette
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");

        assertArrayEquals(BodyInspectComponent.fallbackBodyPartRect(BodyPart.LEFT_HAND),
            BodyInspectComponent.bodyPartRectForTests(BodyPart.LEFT_HAND),
            "part absent from the loaded layout's silhouette must fall back, not vanish or crash");
    }

    // ── unknown plan id: store never resolves a layout, geometry stays on fallback forever ──
    @Test
    void unknownPlanIdKeepsFallbackGeometryForever() {
        BodyPlanLayoutStore.setCurrentPlanId("nonexistent_race");
        assertArrayEquals(BodyInspectComponent.fallbackBodyPartRect(BodyPart.CHEST),
            BodyInspectComponent.bodyPartRectForTests(BodyPart.CHEST));
    }

    // ── synthetic non-humanoid layout: any normalized [0,1] point maps within canvas bounds ──
    @Test
    void syntheticLayoutNormalizedCoordinatesNeverEscapeCanvasBounds() {
        double[][] extremes = {
            {0.0, 0.0}, {1.0, 0.0}, {0.0, 1.0}, {1.0, 1.0}, {0.5, 0.5}
        };
        for (double[] nxy : extremes) {
            int[] xy = BodyInspectComponent.fromNormalizedForTests(nxy[0], nxy[1]);
            assertTrue(xy[0] >= -84 && xy[0] <= 84,
                "x=" + xy[0] + " out of canvas half-width bounds for normalized " + nxy[0]);
            assertTrue(xy[1] >= 0 && xy[1] <= 236,
                "y=" + xy[1] + " out of canvas height bounds for normalized " + nxy[1]);
        }
    }
}
