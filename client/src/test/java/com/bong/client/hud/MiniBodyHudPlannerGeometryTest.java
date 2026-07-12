package com.bong.client.hud;

import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.DerivedAttrFlags;
import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.inventory.model.BodyPart;
import com.bong.client.inventory.model.bodyplan.BodyPlanLayout;
import com.bong.client.inventory.model.bodyplan.PartAnchor;
import com.bong.client.inventory.model.bodyplan.Point2;
import com.bong.client.inventory.state.BodyPlanLayoutStore;
import com.bong.client.network.BodyPlanLayoutHandler;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerPayloadParseResult;
import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
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
        StatusEffectStore.resetForTests();
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

    // ── plan-race-system-v1 P2c: 合成非人 layout 走真实 JSON wire 往返（不是直接 new
    // BodyPlanLayout 对象），验证 handler 解码后写入的 store 驱动渲染同样不越界 ──
    private static JsonObject point(double x, double y) {
        JsonObject p = new JsonObject();
        p.addProperty("x", x);
        p.addProperty("y", y);
        return p;
    }

    private static JsonObject anchor(String partId, double x, double y) {
        JsonObject a = new JsonObject();
        a.addProperty("part_id", partId);
        a.add("point", point(x, y));
        return a;
    }

    /** 6 段合成非人形构型的完整 wire payload（对齐 server 侧
     * `body_plan/layout.rs::synthetic_whale_layout` 的部位命名与坐标结构）。 */
    private static JsonObject syntheticSixPartWirePayload() {
        JsonObject payload = new JsonObject();
        payload.addProperty("body_plan_id", "whale_synthetic");
        payload.add("silhouette", new JsonArray());
        JsonArray anchors = new JsonArray();
        anchors.add(anchor("skull", 0.05, 0.36));
        anchors.add(anchor("torso", 0.2, 0.36));
        anchors.add(anchor("dorsal_fin", 0.35, 0.36));
        anchors.add(anchor("left_pectoral_fin", 0.5, 0.36));
        anchors.add(anchor("right_pectoral_fin", 0.65, 0.36));
        anchors.add(anchor("tail_fin", 0.8, 0.36));
        payload.add("anchors", anchors);
        payload.add("meridian_paths", new JsonArray());
        payload.add("part_display_map", new JsonArray());
        return payload;
    }

    private static void ingestSyntheticSixPartLayoutThroughWire() {
        JsonObject payload = syntheticSixPartWirePayload();
        payload.addProperty("type", "body_plan_layout");
        payload.addProperty("v", 1);
        String json = new Gson().toJson(payload);
        ServerPayloadParseResult parsed = ServerDataEnvelope.parse(json, json.length());
        assertTrue(parsed.isSuccess(), "synthetic wire payload must parse: " + parsed.errorMessage());
        var result = new BodyPlanLayoutHandler().handle(parsed.envelope());
        assertTrue(result.handled(), "BodyPlanLayoutHandler must accept the synthetic 6-part payload: " + result.logMessage());
        BodyPlanLayoutStore.setCurrentPlanId("whale_synthetic");
    }

    @Test
    void syntheticSixPartLayoutIngestedThroughRealWireDecodeRendersWithinPanelBounds() {
        ingestSyntheticSixPartLayoutThroughWire();
        assertEquals("whale_synthetic", BodyPlanLayoutStore.current().bodyPlanId(),
            "handler must have populated the store for the wire's own body_plan_id");

        // 部位 id 是非人形自定义命名（skull/torso/...），resolvePart 类查询都会落空
        // （不属于 16 段人形枚举），因此这里直接用 anchors 的归一化坐标独立验证渲染
        // 数学（与 locatePart 内部同一套 fromNormalized 换算）不越界，锚点数据本身
        // 来自真实 wire 解码，不是测试里直接构造的 Java 对象。
        for (String partId : List.of("skull", "torso", "dorsal_fin", "left_pectoral_fin",
                "right_pectoral_fin", "tail_fin")) {
            BodyPlanLayout layout = BodyPlanLayoutStore.current();
            var a = layout.anchorFor(partId);
            assertTrue(a != null, "wire-decoded layout must retain anchor for " + partId);
            int rx = BX + (int) Math.round(a.point().x() * MiniBodyHudPlanner.BODY_W);
            int ry = BY + (int) Math.round(a.point().y() * MiniBodyHudPlanner.BODY_H);
            assertTrue(rx >= BX && rx <= BX + MiniBodyHudPlanner.BODY_W,
                partId + " wire-decoded anchor x=" + rx + " escaped panel horizontal bounds");
            assertTrue(ry >= BY && ry <= BY + MiniBodyHudPlanner.BODY_H,
                partId + " wire-decoded anchor y=" + ry + " escaped panel vertical bounds");
        }
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

    // ── body_part_resist:/body_part_weaken: 前缀机制在改造后行为不变 ──────────
    // locatePart 换轨（fallback ↔ store-driven）只改坐标，addPartsFromStatus 的
    // 前缀 → BodyPart 分组映射完全独立于本次改造，帧数量必须不受 layout 状态影响。
    @Test
    void bodyPartResistPrefixMechanismUnchangedRegardlessOfLayoutState() {
        CombatHudState hud = CombatHudState.create(0.9f, 0.5f, 0.5f, DerivedAttrFlags.none());
        StatusEffectStore.replace(List.of(
            new StatusEffectStore.Effect("body_part_resist:leg_l", "左腿硬化", StatusEffectStore.Kind.BUFF, 1, 4_000, 0, "", 0)
        ));

        long noLayoutFrames = MiniBodyHudPlanner.buildCommands(hud, null, null, 0L, 1920, 1080).stream()
            .filter(cmd -> cmd.isRect() && cmd.color() == MiniBodyHudPlanner.BODY_PART_RESIST_FRAME_COLOR)
            .count();

        BodyPlanLayoutStore.putLayout(humanoidAnchorsLayout());
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");
        long withLayoutFrames = MiniBodyHudPlanner.buildCommands(hud, null, null, 0L, 1920, 1080).stream()
            .filter(cmd -> cmd.isRect() && cmd.color() == MiniBodyHudPlanner.BODY_PART_RESIST_FRAME_COLOR)
            .count();

        assertEquals(24L, noLayoutFrames, "leg_l maps to thigh/calf/foot × 2 corners = 6 frames × 4 border rects");
        assertEquals(noLayoutFrames, withLayoutFrames,
            "loading a humanoid layout must not change how many resist frames get drawn, only where");
    }

    @Test
    void bodyPartWeakenPrefixMechanismUnchangedRegardlessOfLayoutState() {
        CombatHudState hud = CombatHudState.create(0.9f, 0.5f, 0.5f, DerivedAttrFlags.none());
        StatusEffectStore.replace(List.of(
            new StatusEffectStore.Effect("body_part_weaken:chest", "胸部脆弱", StatusEffectStore.Kind.DEBUFF, 1, 4_000, 0, "", 0)
        ));

        long noLayoutFrames = MiniBodyHudPlanner.buildCommands(hud, null, null, 0L, 1920, 1080).stream()
            .filter(cmd -> cmd.isRect() && cmd.color() == MiniBodyHudPlanner.BODY_PART_WEAKEN_FRAME_COLOR)
            .count();

        BodyPlanLayoutStore.putLayout(humanoidAnchorsLayout());
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");
        long withLayoutFrames = MiniBodyHudPlanner.buildCommands(hud, null, null, 0L, 1920, 1080).stream()
            .filter(cmd -> cmd.isRect() && cmd.color() == MiniBodyHudPlanner.BODY_PART_WEAKEN_FRAME_COLOR)
            .count();

        assertEquals(8L, noLayoutFrames);
        assertEquals(noLayoutFrames, withLayoutFrames,
            "loading a humanoid layout must not change how many weaken frames get drawn, only where");
    }
}
