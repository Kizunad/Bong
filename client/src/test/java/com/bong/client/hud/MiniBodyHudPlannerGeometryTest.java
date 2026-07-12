package com.bong.client.hud;

import com.bong.client.inventory.model.BodyPart;
import com.bong.client.inventory.model.bodyplan.BodyPlanLayout;
import com.bong.client.inventory.model.bodyplan.PartAnchor;
import com.bong.client.inventory.model.bodyplan.Point2;
import com.bong.client.inventory.state.BodyPlanLayoutStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-race-system-v1 P2b — {@code MiniBodyHudPlanner.locatePart} 几何改造回归。
 *
 * <p>与 {@code BodyInspectComponentGeometryTest} 不同：本面板的粗网格（30×75）比例
 * 与它自己的原硬编码红点表并非从同一份权威数据线性推导（原硬编码表是手调近似，
 * 不是从任何 JSON 抽取而来），因此"读 store 后"与"无 layout fallback"在 humanoid
 * 构型上**不再逐值相等**（四肢末端漂移最大 6px，见 MiniBodyHudPlanner.locatePart
 * 类头文档）。本测试锁定的是改造后的**新基线**——数值全部由
 * {@code round(normalized * BODY_W/BODY_H)} 从
 * {@code server/assets/body_plans/layouts/humanoid.json} 的 anchors 独立算出并写死，
 * 任何未来回归都会撞红；同时锁定"无 layout"路径与改造前硬编码表逐值相等（这条
 * 路径的字节码完全未变，理应恒等）。
 */
class MiniBodyHudPlannerGeometryTest {

    private static final int BX = 100;
    private static final int BY = 200;

    @AfterEach
    void tearDown() {
        BodyPlanLayoutStore.resetForTests();
    }

    private static BodyPlanLayout humanoidAnchorsLayout() {
        return new BodyPlanLayout("humanoid", List.of(), List.of(
            new PartAnchor("head", new Point2(0.5, 0.042373)),
            new PartAnchor("neck", new Point2(0.5, 0.127119)),
            new PartAnchor("chest", new Point2(0.5, 0.20339)),
            new PartAnchor("abdomen", new Point2(0.5, 0.347458)),
            new PartAnchor("left_upper_arm", new Point2(0.285714, 0.224576)),
            new PartAnchor("left_forearm", new Point2(0.261905, 0.377119)),
            new PartAnchor("left_hand", new Point2(0.255952, 0.466102)),
            new PartAnchor("right_upper_arm", new Point2(0.714286, 0.224576)),
            new PartAnchor("right_forearm", new Point2(0.738095, 0.377119)),
            new PartAnchor("right_hand", new Point2(0.744048, 0.466102)),
            new PartAnchor("left_thigh", new Point2(0.369048, 0.559322)),
            new PartAnchor("left_calf", new Point2(0.369048, 0.720339)),
            new PartAnchor("left_foot", new Point2(0.386905, 0.805085)),
            new PartAnchor("right_thigh", new Point2(0.630952, 0.559322)),
            new PartAnchor("right_calf", new Point2(0.630952, 0.720339)),
            new PartAnchor("right_foot", new Point2(0.613095, 0.805085))
        ), List.of(), List.of());
    }

    @Test
    void noLayoutLoadedUsesFallbackVerbatim() {
        BodyPlanLayoutStore.resetForTests();
        for (BodyPart bp : BodyPart.values()) {
            assertArrayEquals(
                MiniBodyHudPlanner.fallbackLocatePart(BX, BY, bp),
                MiniBodyHudPlanner.locatePartForTests(BX, BY, bp),
                "no layout loaded: " + bp + " must use the untouched fallback code path verbatim"
            );
        }
    }

    @Test
    void unknownPlanIdUsesFallbackForever() {
        BodyPlanLayoutStore.setCurrentPlanId("nonexistent_race");
        assertArrayEquals(MiniBodyHudPlanner.fallbackLocatePart(BX, BY, BodyPart.CHEST),
            MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.CHEST));
    }

    @Test
    void humanoidLayoutLoadedProducesNewCalibratedBaseline_head() {
        BodyPlanLayoutStore.putLayout(humanoidAnchorsLayout());
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");

        // round(0.5*30)=15, round(0.042373*75)=3 — computed independently from the
        // json anchor values, not copy-pasted from the pre-refactor fallback (which was 4).
        assertArrayEquals(new int[]{BX + 15, BY + 3}, MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.HEAD));
    }

    @Test
    void humanoidLayoutLoadedProducesNewCalibratedBaseline_torsoAndLimbs() {
        BodyPlanLayoutStore.putLayout(humanoidAnchorsLayout());
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");

        assertArrayEquals(new int[]{BX + 15, BY + 10}, MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.NECK));
        assertArrayEquals(new int[]{BX + 15, BY + 15}, MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.CHEST));
        assertArrayEquals(new int[]{BX + 15, BY + 26}, MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.ABDOMEN));
        assertArrayEquals(new int[]{BX + 9, BY + 17}, MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.LEFT_UPPER_ARM));
        assertArrayEquals(new int[]{BX + 22, BY + 35}, MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.RIGHT_HAND));
        assertArrayEquals(new int[]{BX + 12, BY + 60}, MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.LEFT_FOOT));
        assertArrayEquals(new int[]{BX + 18, BY + 60}, MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.RIGHT_FOOT));
    }

    @Test
    void partMissingFromLoadedLayoutFallsBack() {
        // Layout only declares HEAD; every other part must fall back, not vanish/crash.
        BodyPlanLayoutStore.putLayout(new BodyPlanLayout("humanoid", List.of(),
            List.of(new PartAnchor("head", new Point2(0.5, 0.042373))), List.of(), List.of()));
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");

        assertArrayEquals(MiniBodyHudPlanner.fallbackLocatePart(BX, BY, BodyPart.CHEST),
            MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.CHEST));
    }

    // ── synthetic non-humanoid (6-part) layout: dots never escape the mini-body box ──
    @Test
    void syntheticSixPartLayoutDotsStayWithinPanelBounds() {
        BodyPlanLayoutStore.putLayout(new BodyPlanLayout("whale", List.of(), List.of(
            new PartAnchor("head", new Point2(0.0, 0.0)),
            new PartAnchor("chest", new Point2(1.0, 0.0)),
            new PartAnchor("abdomen", new Point2(0.0, 1.0)),
            new PartAnchor("left_upper_arm", new Point2(1.0, 1.0)),
            new PartAnchor("right_upper_arm", new Point2(0.5, 0.5)),
            new PartAnchor("left_thigh", new Point2(0.25, 0.75))
        ), List.of(), List.of()));
        BodyPlanLayoutStore.setCurrentPlanId("whale");

        for (BodyPart bp : List.of(BodyPart.HEAD, BodyPart.CHEST, BodyPart.ABDOMEN,
                BodyPart.LEFT_UPPER_ARM, BodyPart.RIGHT_UPPER_ARM, BodyPart.LEFT_THIGH)) {
            int[] xy = MiniBodyHudPlanner.locatePartForTests(BX, BY, bp);
            assertTrue(xy[0] >= BX && xy[0] <= BX + MiniBodyHudPlanner.BODY_W,
                bp + " x=" + xy[0] + " escaped panel horizontal bounds");
            assertTrue(xy[1] >= BY && xy[1] <= BY + MiniBodyHudPlanner.BODY_H,
                bp + " y=" + xy[1] + " escaped panel vertical bounds");
        }
    }
}
