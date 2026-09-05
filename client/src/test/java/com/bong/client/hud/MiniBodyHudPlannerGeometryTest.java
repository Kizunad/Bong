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
 * plan-race-system-v1 P2 major 修复 — {@code MiniBodyHudPlanner.locatePart} 几何回归。
 *
 * <p>P2b 曾把本面板改成均匀缩放 {@code anchors}（{@code BodyInspectComponent} 的
 * 168×236 精细画布锚点，宽高比 0.71）推导坐标，但本面板自己的粗网格画布是 30×75
 * （宽高比 0.40）——两个消费者画布比例不同，均匀缩放同一套锚点必然在 mini HUD 上
 * 产生 1-6px 像素漂移，违反 plan「首版渲染与现状像素级一致」红线。对抗审查抓出
 * 该 major 问题后改为：{@code BodyPlanLayoutV1} 新增可选的第二套锚点组
 * {@code hud_anchors}，humanoid.json 把本面板改造前的硬编码红点表原样抽取进去
 * （逐值相等，见 server {@code layout.rs} 的
 * {@code humanoid_layout_hud_anchors_pin_mini_hud_fallback_table_verbatim} pin 测试）。
 * {@code locatePart} 现按优先级换轨：① layout 有 {@code hud_anchors} → 用之（humanoid
 * 走这条路径，与旧硬编码表逐像素相等，Δ=0）② 无 {@code hud_anchors}（未来非人 plan
 * 可以不配）→ 回退到 {@code anchors} 缩放推导（本文件的合成非人构型测试组覆盖这条
 * 路径）③ store 无当前 layout / 两组锚点都未声明该部位 → {@link
 * MiniBodyHudPlanner#fallbackLocatePart} 仅视觉 fallback。
 *
 * <p><b>P2c 降级说明（数值对拍而非截图对拍）</b>：plan §P2 测试项首选
 * {@code client/tools} render harness 截图对拍，但该目录现有工具
 * （render_animation.py / render_held_item.py / screenshot_weapon.sh）只覆盖
 * 玩家动画姿态与手持物模型渲染，均不驱动 HUD 渲染层（本 planner 产出的
 * HudRenderCommand 由游戏内 HudRenderer 消费，headless 工具链无对应入口），
 * 因此按 plan 允许的降级路径改用逐坐标数值 pin——buildCommands 输出的 rect
 * 几何完全由这些坐标决定，数值锁死等价于像素级锁死。
 */
class MiniBodyHudPlannerGeometryTest {

    private static final int BX = 100;
    private static final int BY = 200;

    @AfterEach
    void tearDown() {
        BodyPlanLayoutStore.resetForTests();
        StatusEffectStore.resetForTests();
    }

    /**
     * 主锚点组（{@code BodyInspectComponent} 168×236 精细画布锚点，来自真实
     * {@code humanoid.json} 的 {@code anchors} 段）——独立于本面板 {@code hud_anchors}，
     * 用于覆盖「无 hud_anchors → 回退 anchors 缩放推导」换轨分支。
     */
    private static List<PartAnchor> humanoidMainAnchors() {
        return List.of(
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
        );
    }

    /**
     * mini HUD 专用第二锚点组——{@code MiniBodyHudPlanner.fallbackLocatePart}
     * 改造前硬编码表（30×75 画布）原样抽取为归一化坐标（{@code /BODY_W}、
     * {@code /BODY_H}），与 server {@code humanoid.json} 的 {@code hud_anchors}
     * 段逐值对拍来源一致。
     */
    private static List<PartAnchor> humanoidHudAnchors() {
        return List.of(
            new PartAnchor("head", new Point2(15.0 / 30.0, 4.0 / 75.0)),
            new PartAnchor("neck", new Point2(15.0 / 30.0, 9.0 / 75.0)),
            new PartAnchor("chest", new Point2(15.0 / 30.0, 17.0 / 75.0)),
            new PartAnchor("abdomen", new Point2(15.0 / 30.0, 28.0 / 75.0)),
            new PartAnchor("left_upper_arm", new Point2(6.0 / 30.0, 14.0 / 75.0)),
            new PartAnchor("left_forearm", new Point2(6.0 / 30.0, 23.0 / 75.0)),
            new PartAnchor("left_hand", new Point2(6.0 / 30.0, 31.0 / 75.0)),
            new PartAnchor("right_upper_arm", new Point2(24.0 / 30.0, 14.0 / 75.0)),
            new PartAnchor("right_forearm", new Point2(24.0 / 30.0, 23.0 / 75.0)),
            new PartAnchor("right_hand", new Point2(24.0 / 30.0, 31.0 / 75.0)),
            new PartAnchor("left_thigh", new Point2(11.0 / 30.0, 41.0 / 75.0)),
            new PartAnchor("left_calf", new Point2(11.0 / 30.0, 54.0 / 75.0)),
            new PartAnchor("left_foot", new Point2(11.0 / 30.0, 66.0 / 75.0)),
            new PartAnchor("right_thigh", new Point2(18.0 / 30.0, 41.0 / 75.0)),
            new PartAnchor("right_calf", new Point2(18.0 / 30.0, 54.0 / 75.0)),
            new PartAnchor("right_foot", new Point2(18.0 / 30.0, 66.0 / 75.0))
        );
    }

    /** 完整 humanoid layout：主 anchors + hud_anchors 均声明（对齐真实 humanoid.json）。 */
    private static BodyPlanLayout humanoidAnchorsLayout() {
        return new BodyPlanLayout(
            "humanoid", List.of(), humanoidMainAnchors(), List.of(), List.of(), humanoidHudAnchors()
        );
    }

    /** 只声明主 anchors、没有 hud_anchors 的 humanoid layout——覆盖回退到 anchors 缩放推导的换轨分支。 */
    private static BodyPlanLayout humanoidMainAnchorsOnlyLayout() {
        return new BodyPlanLayout("humanoid", List.of(), humanoidMainAnchors(), List.of(), List.of());
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

    // ── plan-race-system-v1 P2 major 修复：humanoid + hud_anchors → 与改造前
    // 硬编码表逐像素相等（Δ=0），锁死 plan「首版渲染与现状像素级一致」红线 ──

    @Test
    void humanoidLayoutWithHudAnchorsIsPixelIdenticalToPreRefactorFallback_allParts() {
        BodyPlanLayoutStore.putLayout(humanoidAnchorsLayout());
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");

        for (BodyPart bp : BodyPart.values()) {
            assertArrayEquals(
                MiniBodyHudPlanner.fallbackLocatePart(BX, BY, bp),
                MiniBodyHudPlanner.locatePartForTests(BX, BY, bp),
                "humanoid layout with hud_anchors must reproduce the pre-refactor hardcoded "
                    + "table verbatim (Δ=0) for " + bp + " — hud_anchors was extracted from "
                    + "that exact table, this is the plan's pixel-parity red line"
            );
        }
    }

    @Test
    void humanoidLayoutWithHudAnchorsMatchesExpectedFallbackConstants_spotCheck() {
        // 外部锚点（不从实现反推）：直接写死 MiniBodyHudPlanner.fallbackLocatePart 改造前
        // 硬编码表的原始数值，逐个 spot check（对拍上面 allParts 循环断言用的同一份数据源，
        // 双保险防止 fallbackLocatePart 本身被意外改动而让 allParts 测试跟着漂移失去意义）。
        BodyPlanLayoutStore.putLayout(humanoidAnchorsLayout());
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");

        assertArrayEquals(new int[]{BX + 15, BY + 4}, MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.HEAD));
        assertArrayEquals(new int[]{BX + 15, BY + 9}, MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.NECK));
        assertArrayEquals(new int[]{BX + 15, BY + 17}, MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.CHEST));
        assertArrayEquals(new int[]{BX + 15, BY + 28}, MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.ABDOMEN));
        assertArrayEquals(new int[]{BX + 6, BY + 14}, MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.LEFT_UPPER_ARM));
        assertArrayEquals(new int[]{BX + 24, BY + 31}, MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.RIGHT_HAND));
        assertArrayEquals(new int[]{BX + 11, BY + 66}, MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.LEFT_FOOT));
        assertArrayEquals(new int[]{BX + 18, BY + 66}, MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.RIGHT_FOOT));
    }

    // ── 无 hud_anchors（只声明主 anchors）时的回退换轨分支：缩放推导，允许与
    // 硬编码表存在几像素漂移（未来非人 plan 没有另一份权威 mini HUD 像素表可抽取时的
    // 唯一合法路径） ──

    @Test
    void humanoidLayoutWithoutHudAnchorsFallsBackToScaledMainAnchors_head() {
        BodyPlanLayoutStore.putLayout(humanoidMainAnchorsOnlyLayout());
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");

        // round(0.5*30)=15, round(0.042373*75)=3 — 从主 anchors 独立缩放推导，
        // 与 hud_anchors 路径的 4 不同（无 hud_anchors 时的合法漂移）。
        assertArrayEquals(new int[]{BX + 15, BY + 3}, MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.HEAD));
    }

    @Test
    void humanoidLayoutWithoutHudAnchorsFallsBackToScaledMainAnchors_torsoAndLimbs() {
        BodyPlanLayoutStore.putLayout(humanoidMainAnchorsOnlyLayout());
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
    void hudAnchorTakesPriorityOverMainAnchorWhenBothDeclareSamePart() {
        // 两组锚点都声明同一部位时，hud_anchors 优先——这是换轨顺序的核心契约，
        // 不能靠上面两组测试间接推断（万一某天顺序被颠倒，这两组测试各自单独看仍会
        // 通过，只有本测试直接锁死"谁赢"）。
        BodyPlanLayoutStore.putLayout(new BodyPlanLayout("humanoid", List.of(),
            List.of(new PartAnchor("chest", new Point2(0.9, 0.9))),
            List.of(), List.of(),
            List.of(new PartAnchor("chest", new Point2(0.1, 0.1)))
        ));
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");

        assertArrayEquals(
            new int[]{BX + (int) Math.round(0.1 * MiniBodyHudPlanner.BODY_W), BY + (int) Math.round(0.1 * MiniBodyHudPlanner.BODY_H)},
            MiniBodyHudPlanner.locatePartForTests(BX, BY, BodyPart.CHEST),
            "hud_anchors must win over anchors when both declare the same part"
        );
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
            .filter(cmd -> cmd.isVector() && cmd.color() == MiniBodyHudPlanner.BODY_PART_RESIST_FRAME_COLOR)
            .count();

        BodyPlanLayoutStore.putLayout(humanoidAnchorsLayout());
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");
        long withLayoutFrames = MiniBodyHudPlanner.buildCommands(hud, null, null, 0L, 1920, 1080).stream()
            .filter(cmd -> cmd.isVector() && cmd.color() == MiniBodyHudPlanner.BODY_PART_RESIST_FRAME_COLOR)
            .count();

        assertEquals(24L, noLayoutFrames, "leg_l maps to thigh/calf/foot × 2 corners = 6 frames × 4 border rects");
        assertEquals(noLayoutFrames, withLayoutFrames,
            "loading a humanoid layout must not change how many resist frames get drawn, only where");

        MiniBodyHudPlanner.buildCommands(hud, null, null, 0L, 1920, 1080).stream()
            .filter(cmd -> cmd.color() == MiniBodyHudPlanner.BODY_PART_RESIST_FRAME_COLOR)
            .forEach(cmd -> {
                assertTrue(cmd.isVector(), "动态部位边框必须走 fill.svg vector 命令");
                assertEquals("fill", cmd.text(), "部位边框只能使用白名单 fill 资源");
                assertTrue((cmd.width() == 1 && cmd.height() >= 1)
                        || (cmd.height() == 1 && cmd.width() >= 1),
                    "部位边框必须由水平或垂直细条组成");
            });
    }

    @Test
    void bodyPartWeakenPrefixMechanismUnchangedRegardlessOfLayoutState() {
        CombatHudState hud = CombatHudState.create(0.9f, 0.5f, 0.5f, DerivedAttrFlags.none());
        StatusEffectStore.replace(List.of(
            new StatusEffectStore.Effect("body_part_weaken:chest", "胸部脆弱", StatusEffectStore.Kind.DEBUFF, 1, 4_000, 0, "", 0)
        ));

        long noLayoutFrames = MiniBodyHudPlanner.buildCommands(hud, null, null, 0L, 1920, 1080).stream()
            .filter(cmd -> cmd.isVector() && cmd.color() == MiniBodyHudPlanner.BODY_PART_WEAKEN_FRAME_COLOR)
            .count();

        BodyPlanLayoutStore.putLayout(humanoidAnchorsLayout());
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");
        long withLayoutFrames = MiniBodyHudPlanner.buildCommands(hud, null, null, 0L, 1920, 1080).stream()
            .filter(cmd -> cmd.isVector() && cmd.color() == MiniBodyHudPlanner.BODY_PART_WEAKEN_FRAME_COLOR)
            .count();

        assertEquals(8L, noLayoutFrames);
        assertEquals(noLayoutFrames, withLayoutFrames,
            "loading a humanoid layout must not change how many weaken frames get drawn, only where");
    }
}
