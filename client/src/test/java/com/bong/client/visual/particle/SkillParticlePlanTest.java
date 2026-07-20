package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Optional;
import java.util.OptionalInt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-skill-anim-fidelity-v1 P5 —— 三个新 VfxPlayer 的**真实发射输出**逐颗断言。
 *
 * <p><b>为什么必须有这一份</b>（review r1 #3 的根因）：{@link SkillVfxPlayerFormTest} 只测
 * {@code Form} 枚举的 id→形态映射，<b>从未执行或观察实际 spawn</b>——于是「穴位点 lifetime 擅自
 * +6」「切向环绕实为切向抛射」两处实现偏离在全绿门禁下溜了过去。本文件消费各 player 的纯函数
 * {@code plan(payload)}，逐颗断言粒子基类 / 数量 / 初始位置 / 速度向量 / lifetime / 颜色 /
 * 贴图 provider，让同类偏离立刻撞红。
 *
 * <p><b>覆盖边界</b>：{@code play()} 里剩下的唯一未覆盖分支是 {@code client.world == null} 的早退
 * ——它需要真 {@code MinecraftClient}，纯 JVM 单测造不出。这不是遗漏而是切分的结果：
 * {@code plan()} 与 world 完全无关（不读任何 world 状态），world 只在
 * {@link SkillParticleSpawner} 实例化粒子时才用到，那一行早退无逻辑可漂。
 */
public class SkillParticlePlanTest {

    private static final double EPS = 1e-9;
    /** 三招 payload 共用的原点，各分量互不相等，位置断言错位时能一眼看出错在哪个轴。 */
    private static final double[] ORIGIN = { 10.0, 64.0, -20.0 };

    // ─── ZhenmaiPulsePlayer：5 招真实发射 ────────────────────────────────────────

    @Test
    void zhenmaiParryEmitsForwardPulsesPlusStaticAcupoints() {
        List<SkillParticleSpawn> spawns =
            ZhenmaiPulsePlayer.plan(payload(ZhenmaiPulsePlayer.PARRY_FLASH));

        assertEquals(8 + 3, spawns.size(),
            "parry 应发 8 条脉冲 + 3 个穴位点（plan §P5.1 ①），实际 " + spawns.size());

        List<SkillParticleSpawn> pulses = spawns.subList(0, 8);
        for (SkillParticleSpawn spawn : pulses) {
            SkillParticleSpawn.Pulse pulse = assertInstanceOf(SkillParticleSpawn.Pulse.class, spawn,
                "parry 脉冲应是 BongLineParticle 描述符（Pulse）");
            assertEquals(0xD4AF6A, pulse.rgb(), "parry 应持金脉 anchor #D4AF6A");
            assertEquals(20, pulse.maxAge(), "parry lifetime 应为 spec 的 20t");
            assertSame(SkillParticleSpawn.Sheet.QI_AURA, pulse.sheet(), "脉冲复用 qi_aura 贴图");
            // 缺省 direction 回退 +X，故前向速度整体压在 X 轴上。
            assertEquals(0.35, pulse.velocityX(), EPS, "parry 前向速度应为 spec 的 0.35 格/t");
            assertEquals(0.0, pulse.velocityY(), EPS);
            assertEquals(0.0, pulse.velocityZ(), EPS);
        }

        for (SkillParticleSpawn spawn : spawns.subList(8, 11)) {
            SkillParticleSpawn.Point point = assertInstanceOf(
                SkillParticleSpawn.Point.class, spawn, "穴位点应是 BongSpriteParticle 描述符（Point）");
            assertEquals(20, point.maxAge(),
                "review r1 #1：穴位点 lifetime 必须与脉冲一致（20t），"
                    + "此前实现私自 +6 = 26t 而 spec 写的是 20t；实际 " + point.maxAge());
            assertEquals(0.0, point.velocityY(), EPS,
                "review r1 #1：plan §P5.1 ① parry 行明写「穴位点静止」，"
                    + "此前实现给了 0.008 上浮；实际 vy=" + point.velocityY());
            assertSame(SkillParticleSpawn.Sheet.LINGQI_RIPPLE, point.sheet(),
                "穴位点复用 lingqi_ripple 贴图");
        }
    }

    /** 穴位点漂移必须逐招显式配置——不是「统一给个上浮」，也不是「一律静止」。 */
    @Test
    void zhenmaiAcupointDriftMatchesPerFormSpec() {
        assertAcupointDrift(ZhenmaiPulsePlayer.PARRY_FLASH, 3, 0.0);
        assertAcupointDrift(ZhenmaiPulsePlayer.NEUTRALIZE_DUST, 4, -0.02);
        assertAcupointDrift(ZhenmaiPulsePlayer.MULTIPOINT_RING, 8, 0.0);
        assertAcupointDrift(ZhenmaiPulsePlayer.HARDEN_SHELL, 6, 0.01);
        assertAcupointDrift(ZhenmaiPulsePlayer.SEVER_SNAP, 2, 0.0);
    }

    private static void assertAcupointDrift(Identifier eventId, int expectedPoints, double drift) {
        List<SkillParticleSpawn> points = ZhenmaiPulsePlayer.plan(payload(eventId)).stream()
            .filter(s -> s instanceof SkillParticleSpawn.Point)
            .toList();
        assertEquals(expectedPoints, points.size(),
            eventId + " 的穴位点数应为 " + expectedPoints + "，实际 " + points.size());
        for (SkillParticleSpawn point : points) {
            assertEquals(drift, point.velocityY(), EPS,
                eventId + " 的穴位点漂移应为 spec 的 " + drift + " 格/t，实际 " + point.velocityY());
            assertEquals(0.0, point.velocityX(), EPS, "穴位点不做水平移动");
            assertEquals(0.0, point.velocityZ(), EPS, "穴位点不做水平移动");
        }
    }

    @Test
    void zhenmaiNeutralizeSpreadsOutwardAndSinks() {
        List<SkillParticleSpawn> spawns =
            ZhenmaiPulsePlayer.plan(payload(ZhenmaiPulsePlayer.NEUTRALIZE_DUST));

        assertEquals(10 + 4, spawns.size(), "neutralize 应发 10 脉冲 + 4 穴位点");
        SkillParticleSpawn.Pulse first = assertInstanceOf(
            SkillParticleSpawn.Pulse.class, spawns.get(0));
        assertEquals(0xC9A05C, first.rgb(), "neutralize 应持沉金 #C9A05C");
        assertEquals(-0.02, first.velocityY(), EPS, "卸力散尘应下沉 -0.02 格/t");
        // i=0 → angle 0 → 正 X 方向铺开，速度沿 +X 外散。
        assertEquals(ORIGIN[0] + 0.40, first.x(), EPS, "起始应铺在半径 0.40 格处");
        assertEquals(0.12, first.velocityX(), EPS, "径向外散速度应为 spec 的 0.12 格/t");
    }

    @Test
    void zhenmaiHardenContractsInwardAcrossTwoLayers() {
        List<SkillParticleSpawn> spawns =
            ZhenmaiPulsePlayer.plan(payload(ZhenmaiPulsePlayer.HARDEN_SHELL));

        assertEquals(8 + 6, spawns.size(), "harden 应发 8 脉冲 + 6 穴位点");
        SkillParticleSpawn.Pulse inner = assertInstanceOf(
            SkillParticleSpawn.Pulse.class, spawns.get(0));
        SkillParticleSpawn.Pulse outer = assertInstanceOf(
            SkillParticleSpawn.Pulse.class, spawns.get(1));

        assertEquals(0xB8944F, inner.rgb(), "harden 应持暗金 #B8944F");
        assertEquals(0.01, inner.velocityY(), EPS, "护壳应微上浮 0.01 格/t");
        // 奇偶分内外两层：内层 radius×1.0，外层 radius×1.5 且抬高 0.35。
        assertEquals(ORIGIN[0] + 0.45, inner.x(), EPS, "内层应在半径 0.45 格");
        assertEquals(ORIGIN[1], inner.y(), EPS, "内层不抬高");
        assertEquals(ORIGIN[1] + 0.35, outer.y(), EPS, "外层应抬高 0.35 格构成双层护壳");
        assertTrue(inner.velocityX() < 0.0,
            "向心内收：+X 侧的粒子速度应指向 -X，实际 " + inner.velocityX());
    }

    @Test
    void zhenmaiSeverIsTheFastestForwardBurst() {
        List<SkillParticleSpawn> spawns =
            ZhenmaiPulsePlayer.plan(payload(ZhenmaiPulsePlayer.SEVER_SNAP));

        assertEquals(18 + 2, spawns.size(), "sever 应发 18 脉冲 + 2 穴位点（爆点而非环）");
        SkillParticleSpawn.Pulse pulse = assertInstanceOf(
            SkillParticleSpawn.Pulse.class, spawns.get(0));
        assertEquals(0xF2D68A, pulse.rgb(), "sever 应持最亮金 #F2D68A");
        assertEquals(0.55, pulse.velocityX(), EPS, "断脉爆冲速度应为 spec 的 0.55 格/t");
    }

    // ─── review r1 #2：真环绕（三处）───────────────────────────────────────────────

    @Test
    void zhenmaiMultipointOrbitsInsteadOfFlyingOffTangentially() {
        List<SkillParticleSpawn> spawns =
            ZhenmaiPulsePlayer.plan(payload(ZhenmaiPulsePlayer.MULTIPOINT_RING));

        assertEquals(16 + 8, spawns.size(), "multipoint 应发 16 脉冲 + 8 穴位点");

        for (int i = 0; i < 16; i++) {
            SkillParticleSpawn.OrbitPulse orbit = assertInstanceOf(
                SkillParticleSpawn.OrbitPulse.class, spawns.get(i),
                "review r1 #2：多点连环是「切向环绕」，必须是 OrbitPulse 真环绕描述符；"
                    + "普通 Pulse 只有切向初速度 = 沿切线直线飞离");
            assertEquals(ORIGIN[0], orbit.orbit().centerX(), EPS, "环心应锚在 origin");
            assertEquals(ORIGIN[2], orbit.orbit().centerZ(), EPS, "环心应锚在 origin");
            assertEquals(0.50, orbit.orbit().radius(), EPS, "环绕半径应为 spec 的 0.50 格");
            assertEquals(0.10, orbit.orbit().tangentialSpeed(), EPS,
                "环绕线速度应为 spec 的 0.10 格/t");
            assertEquals(0.0, orbit.orbit().verticalSpeed(), EPS, "腰高环不做垂直漂移");
            assertEquals(0xE0C27E, orbit.rgb(), "multipoint 应持亮金 #E0C27E");
            assertEquals(20, orbit.maxAge());
        }
        // 16 颗等角铺满一圈，起始角互不相同才读得出「多点」。
        assertEquals(0.0,
            assertInstanceOf(SkillParticleSpawn.OrbitPulse.class, spawns.get(0))
                .orbit().startAngle(), EPS);
        assertEquals(Math.PI * 2.0 / 16,
            assertInstanceOf(SkillParticleSpawn.OrbitPulse.class, spawns.get(1))
                .orbit().startAngle(), EPS);
    }

    @Test
    void burstCounterflowOrbitsTheBodyAtTwoHeights() {
        List<SkillParticleSpawn> spawns =
            BurstMeridianFamilyPlayer.plan(payload(BurstMeridianFamilyPlayer.NI_MAI_HU_TI));

        assertEquals(14, spawns.size(), "逆脉护体应发 14 颗（plan §P5.1 ②）");
        for (int i = 0; i < spawns.size(); i++) {
            SkillParticleSpawn.OrbitPoint orbit = assertInstanceOf(
                SkillParticleSpawn.OrbitPoint.class, spawns.get(i),
                "review r1 #2：plan P5 原文写的是「Sprite 环绕」，必须是 OrbitPoint 真环绕");
            assertEquals(0.55, orbit.orbit().radius(), EPS, "贴身半径应为 spec 的 0.55 格");
            assertEquals(0.08, orbit.orbit().tangentialSpeed(), EPS, "环绕线速度 0.08 格/t");
            assertEquals(0.015, orbit.orbit().verticalSpeed(), EPS, "上浮 0.015 格/t");
            assertEquals(60, orbit.maxAge(), "护体粒子应持续到 60t buff 窗口结束");
            assertEquals(0xC58B3F, orbit.rgb(), "爆脉家族色 #C58B3F");
            // 奇偶分上下两环。
            double expectedCenterY = ORIGIN[1] + ((i % 2 == 0) ? -0.25 : 0.45);
            assertEquals(expectedCenterY, orbit.orbit().centerY(), EPS,
                "第 " + i + " 颗应落在双高度环的 " + ((i % 2 == 0) ? "下" : "上") + "环");
        }
    }

    @Test
    void npcDefenseRingOrbitsRatherThanScattering() {
        List<SkillParticleSpawn> spawns =
            NpcSkillAuraPlayer.plan(payload(NpcSkillAuraPlayer.BUFF_DEFENSE));

        assertEquals(12, spawns.size());
        for (SkillParticleSpawn spawn : spawns) {
            SkillParticleSpawn.OrbitPoint orbit = assertInstanceOf(
                SkillParticleSpawn.OrbitPoint.class, spawn,
                "review r1 #2：护体环必须真环绕，切向抛射会读成「炸开」而非「护住」");
            assertEquals(0.6, orbit.orbit().radius(), EPS);
            assertEquals(0.06, orbit.orbit().tangentialSpeed(), EPS);
            assertEquals(0.0, orbit.orbit().verticalSpeed(), EPS, "护环是严格水平环");
            assertEquals(0x5BA8C9, orbit.rgb(), "护体应持青蓝 #5BA8C9");
            assertEquals(40, orbit.maxAge());
            assertSame(SkillParticleSpawn.Sheet.LINGQI_RIPPLE, orbit.sheet());
        }
    }

    /**
     * 环绕的**行为**判据：推进整个 lifetime 后半径仍然恒定。
     *
     * <p>这是 review r1 #2 的直接回归锁——切向抛射实现下，到中心的距离会随 tick 线性增长
     * （60t × 0.08 格/t ≈ 4.8 格，粒子早飞出体表几个身位）。
     */
    @Test
    void orbitingSpawnsKeepRadiusForTheirWholeLifetime() {
        record Case(String name, SkillParticleSpawn.OrbitSpec orbit, int lifetime) {}

        List<Case> cases = List.of(
            new Case("zhenmai.multipoint", orbitOf(
                ZhenmaiPulsePlayer.plan(payload(ZhenmaiPulsePlayer.MULTIPOINT_RING)).get(0)), 20),
            new Case("burst.ni_mai_hu_ti", orbitOf(
                BurstMeridianFamilyPlayer.plan(
                    payload(BurstMeridianFamilyPlayer.NI_MAI_HU_TI)).get(0)), 60),
            new Case("npc.buff_defense", orbitOf(
                NpcSkillAuraPlayer.plan(payload(NpcSkillAuraPlayer.BUFF_DEFENSE)).get(0)), 40)
        );

        for (Case testCase : cases) {
            OrbitPath path = new OrbitPath(testCase.orbit());
            double radius = testCase.orbit().radius();
            for (int tick = 0; tick < testCase.lifetime(); tick++) {
                path.advance();
                assertEquals(radius, path.horizontalRadius(), 1e-9,
                    testCase.name() + " 在第 " + tick + " tick 半径漂到了 "
                        + path.horizontalRadius() + "（应恒为 " + radius
                        + "）——环绕退化成了抛射");
            }
            // 走完整个 lifetime 后确实转过了角度（而不是原地不动冒充"半径恒定"）。
            assertTrue(Math.abs(path.angle() - testCase.orbit().startAngle()) > 1e-3,
                testCase.name() + " 走完 lifetime 角位置几乎没变——粒子钉住不动不是环绕");
        }
    }

    private static SkillParticleSpawn.OrbitSpec orbitOf(SkillParticleSpawn spawn) {
        if (spawn instanceof SkillParticleSpawn.OrbitPoint point) {
            return point.orbit();
        }
        if (spawn instanceof SkillParticleSpawn.OrbitPulse pulse) {
            return pulse.orbit();
        }
        throw new AssertionError("期望环绕描述符，实际 " + spawn.getClass().getSimpleName());
    }

    // ─── BurstMeridianFamilyPlayer：另两招 ───────────────────────────────────────

    @Test
    void burstImpactRingHugsTheGroundAndSpins() {
        List<SkillParticleSpawn> spawns =
            BurstMeridianFamilyPlayer.plan(payload(BurstMeridianFamilyPlayer.TIE_SHAN_KAO));

        assertEquals(10, spawns.size(), "贴山靠应发 10 颗地贴");
        // 缺省 strength 0.95 → 半径 0.30 + 0.45×0.95 = 0.7275。
        double expectedHalfSize = 0.30 + 0.45 * 0.95;
        for (SkillParticleSpawn spawn : spawns) {
            SkillParticleSpawn.Decal decal = assertInstanceOf(
                SkillParticleSpawn.Decal.class, spawn, "撞击环应是 BongGroundDecalParticle 描述符");
            assertEquals(expectedHalfSize, decal.halfSize(), EPS,
                "撞击环半径应为 0.30 + 0.45×strength");
            assertEquals(0.05, decal.spinRadPerTick(), EPS, "自旋应为 spec 的 0.05 rad/t");
            assertEquals(ORIGIN[1] - 1.0 + 0.03, decal.y(), EPS,
                "server origin 抬到胸口，撞击环须落回脚下（-1 格）再抬 0.03 防 z-fighting");
            assertEquals(0.0, decal.velocityX(), EPS, "贴地静止");
            assertEquals(0.0, decal.velocityY(), EPS, "贴地静止");
            assertEquals(0.0, decal.velocityZ(), EPS, "贴地静止");
            assertEquals(16, decal.maxAge(), "payload 未带 duration 时回退 16t");
            assertSame(SkillParticleSpawn.Sheet.LINGQI_RIPPLE, decal.sheet());
        }
    }

    @Test
    void burstDashTrailsBackwardsBehindTheCaster() {
        // 朝 +Z 突进，残影必须落在 -Z 侧。
        List<SkillParticleSpawn> spawns = BurstMeridianFamilyPlayer.plan(payload(
            BurstMeridianFamilyPlayer.XUE_BENG_BU,
            Optional.of(new double[] { 0.0, 0.0, 1.0 }),
            OptionalInt.empty(), Optional.empty(), OptionalInt.empty(), OptionalInt.empty()));

        assertEquals(12, spawns.size(), "血崩步应发 12 条残影");
        for (SkillParticleSpawn spawn : spawns) {
            SkillParticleSpawn.Ribbon ribbon = assertInstanceOf(
                SkillParticleSpawn.Ribbon.class, spawn, "残影应是 BongRibbonParticle 描述符");
            assertEquals(-0.25, ribbon.velocityZ(), EPS,
                "残影须沿 direction 反向拖出（+Z 突进 → -Z 拖尾），顺向会读成「往前喷气」");
            assertEquals(0.01, ribbon.velocityY(), EPS);
            assertEquals(14, ribbon.maxAge(), "payload 未带 duration 时回退 14t");
            assertTrue(ribbon.z() < ORIGIN[2],
                "残影起点应偏向身后，实际 z=" + ribbon.z() + " 应 < " + ORIGIN[2]);
            assertSame(SkillParticleSpawn.Sheet.QI_AURA, ribbon.sheet());
        }
    }

    // ─── NpcSkillAuraPlayer：另两招 ──────────────────────────────────────────────

    @Test
    void npcHealRisesAsAColumnOfPoints() {
        List<SkillParticleSpawn> spawns =
            NpcSkillAuraPlayer.plan(payload(NpcSkillAuraPlayer.HEAL_BASIC));

        assertEquals(12, spawns.size());
        for (SkillParticleSpawn spawn : spawns) {
            SkillParticleSpawn.Point point =
                assertInstanceOf(SkillParticleSpawn.Point.class, spawn);
            assertEquals(0.03, point.velocityY(), EPS, "回血光点应上浮 0.03 格/t");
            assertEquals(0.0, point.velocityX(), EPS, "回血柱不做水平移动");
            assertEquals(0xA8E6CF, point.rgb(), "回血应持薄荷绿 #A8E6CF");
            assertEquals(40, point.maxAge());
        }
        // 分四段身高错落，读成一根柱子而不是一个平面圈。
        assertEquals(ORIGIN[1] - 0.5, spawns.get(0).y(), EPS);
        assertEquals(ORIGIN[1] - 0.5 + 0.22, spawns.get(1).y(), EPS);
        assertEquals(ORIGIN[1] - 0.5, spawns.get(4).y(), EPS, "第 5 颗应回到最低段（i%4）");
    }

    @Test
    void npcSpeedDustRushesOutwardAsLineParticles() {
        List<SkillParticleSpawn> spawns =
            NpcSkillAuraPlayer.plan(payload(NpcSkillAuraPlayer.BUFF_SPEED));

        assertEquals(12, spawns.size());
        SkillParticleSpawn.Pulse first =
            assertInstanceOf(SkillParticleSpawn.Pulse.class, spawns.get(0),
                "疾行尘应是线粒子，与另两招的点粒子形态区分");
        assertEquals(0xE3C766, first.rgb(), "加速应持麦黄 #E3C766");
        assertEquals(0.22, first.velocityX(), EPS, "水平外冲应为 spec 的 0.22 格/t");
        assertEquals(ORIGIN[1] - 0.6, first.y(), EPS, "疾行尘贴脚边");
        assertEquals(40, first.maxAge());
    }

    // ─── 边界 / 缺省 / 错误分支 ──────────────────────────────────────────────────

    @Test
    void unknownEventIdsProduceNoParticlesInsteadOfThrowing() {
        Identifier alien = new Identifier("bong", "definitely_not_a_p5_skill");
        assertTrue(ZhenmaiPulsePlayer.plan(payload(alien)).isEmpty(),
            "注册表把别家 id 派进来时应静默跳过而不是画错东西");
        assertTrue(BurstMeridianFamilyPlayer.plan(payload(alien)).isEmpty());
        assertTrue(NpcSkillAuraPlayer.plan(payload(alien)).isEmpty());

        // 交叉投递：每个 player 都不得接管别家的 id。
        assertTrue(ZhenmaiPulsePlayer.plan(payload(NpcSkillAuraPlayer.HEAL_BASIC)).isEmpty());
        assertTrue(BurstMeridianFamilyPlayer.plan(
            payload(ZhenmaiPulsePlayer.PARRY_FLASH)).isEmpty());
        assertTrue(NpcSkillAuraPlayer.plan(
            payload(BurstMeridianFamilyPlayer.TIE_SHAN_KAO)).isEmpty());
        // 崩拳本尊归 BurstMeridianBengQuanPlayer，家族 player 不接管。
        assertTrue(BurstMeridianFamilyPlayer.plan(
            payload(BurstMeridianBengQuanPlayer.EVENT_ID)).isEmpty());
    }

    @Test
    void payloadOverridesBeatFormDefaults() {
        List<SkillParticleSpawn> spawns = ZhenmaiPulsePlayer.plan(payload(
            ZhenmaiPulsePlayer.PARRY_FLASH,
            Optional.empty(),
            OptionalInt.of(0x123456),
            Optional.of(1.0),
            OptionalInt.of(5),
            OptionalInt.of(77)));

        assertEquals(5 + 3, spawns.size(), "payload.count 应驱动脉冲数（穴位点数由形态自持）");
        for (SkillParticleSpawn spawn : spawns) {
            assertEquals(0x123456, spawn.rgb(), "payload 显式颜色应覆盖 Form 兜底色");
            assertEquals(77, spawn.maxAge(),
                "payload 显式 duration 应覆盖默认 lifetime——穴位点也一样（不再私自 +6）");
        }
    }

    @Test
    void particleCountsAreClampedAtBothEnds() {
        // 下限：count=0 对每个 player 都不得产出 0 条主粒子。
        assertEquals(1 + 3, ZhenmaiPulsePlayer.plan(countPayload(
            ZhenmaiPulsePlayer.PARRY_FLASH, 0)).size(), "真脉脉冲下限 1");
        assertEquals(4, BurstMeridianFamilyPlayer.plan(countPayload(
            BurstMeridianFamilyPlayer.TIE_SHAN_KAO, 0)).size(),
            "环形招下限 4——低于此读不出「环」");
        assertEquals(2, BurstMeridianFamilyPlayer.plan(countPayload(
            BurstMeridianFamilyPlayer.XUE_BENG_BU, 1)).size(), "线性残影下限 2");
        assertEquals(4, BurstMeridianFamilyPlayer.plan(countPayload(
            BurstMeridianFamilyPlayer.NI_MAI_HU_TI, 1)).size(), "环形招下限 4");
        assertEquals(1, NpcSkillAuraPlayer.plan(countPayload(
            NpcSkillAuraPlayer.HEAL_BASIC, 0)).size(), "NPC 招下限 1");

        // 上限：异常放大的 count 不得放出粒子风暴。
        assertEquals(48 + 3, ZhenmaiPulsePlayer.plan(countPayload(
            ZhenmaiPulsePlayer.PARRY_FLASH, 9999)).size(), "真脉脉冲上限 48");
        assertEquals(48, BurstMeridianFamilyPlayer.plan(countPayload(
            BurstMeridianFamilyPlayer.NI_MAI_HU_TI, 9999)).size(), "爆脉上限 48");
        assertEquals(48, NpcSkillAuraPlayer.plan(countPayload(
            NpcSkillAuraPlayer.BUFF_DEFENSE, 9999)).size(), "NPC 上限 48");
    }

    /** count=1 时 FORWARD 形态的扇面展开公式含 {@code count-1} 除法，必须不炸也不产 NaN。 */
    @Test
    void singlePulseDoesNotDivideByZeroInSpreadFormulas() {
        for (SkillParticleSpawn spawn
            : ZhenmaiPulsePlayer.plan(countPayload(ZhenmaiPulsePlayer.PARRY_FLASH, 1))) {
            assertAllFinite(spawn, "zhenmai.parry count=1");
        }
        for (SkillParticleSpawn spawn
            : BurstMeridianFamilyPlayer.plan(countPayload(
                BurstMeridianFamilyPlayer.XUE_BENG_BU, 1))) {
            assertAllFinite(spawn, "burst.xue_beng_bu count=1");
        }
    }

    @Test
    void degenerateDirectionFallsBackWithoutProducingNaN() {
        for (double[] badDirection : new double[][] {
            { 0.0, 0.0, 0.0 },
            { Double.NaN, 1.0, 0.0 },
            { Double.POSITIVE_INFINITY, 0.0, 0.0 },
        }) {
            List<SkillParticleSpawn> zhenmai = ZhenmaiPulsePlayer.plan(payload(
                ZhenmaiPulsePlayer.PARRY_FLASH, Optional.of(badDirection),
                OptionalInt.empty(), Optional.empty(), OptionalInt.empty(), OptionalInt.empty()));
            assertFalse(zhenmai.isEmpty());
            for (SkillParticleSpawn spawn : zhenmai) {
                assertAllFinite(spawn, "zhenmai.parry 退化 direction");
            }

            List<SkillParticleSpawn> burst = BurstMeridianFamilyPlayer.plan(payload(
                BurstMeridianFamilyPlayer.XUE_BENG_BU, Optional.of(badDirection),
                OptionalInt.empty(), Optional.empty(), OptionalInt.empty(), OptionalInt.empty()));
            assertFalse(burst.isEmpty());
            for (SkillParticleSpawn spawn : burst) {
                assertAllFinite(spawn, "burst.xue_beng_bu 退化 direction");
            }
        }
    }

    @Test
    void strengthBoundsStayWithinRenderableRanges() {
        for (double strength : new double[] { -5.0, 0.0, 1.0, 9.0 }) {
            List<SkillParticleSpawn> spawns = ZhenmaiPulsePlayer.plan(payload(
                ZhenmaiPulsePlayer.PARRY_FLASH, Optional.empty(), OptionalInt.empty(),
                Optional.of(strength), OptionalInt.empty(), OptionalInt.empty()));
            for (SkillParticleSpawn spawn : spawns) {
                assertTrue(spawn.alpha() >= 0.1f && spawn.alpha() <= 1.0f,
                    "strength=" + strength + " 时 alpha 越界：" + spawn.alpha());
                assertAllFinite(spawn, "zhenmai.parry strength=" + strength);
            }

            List<SkillParticleSpawn> impact = BurstMeridianFamilyPlayer.plan(payload(
                BurstMeridianFamilyPlayer.TIE_SHAN_KAO, Optional.empty(), OptionalInt.empty(),
                Optional.of(strength), OptionalInt.empty(), OptionalInt.empty()));
            for (SkillParticleSpawn spawn : impact) {
                SkillParticleSpawn.Decal decal =
                    assertInstanceOf(SkillParticleSpawn.Decal.class, spawn);
                assertTrue(decal.halfSize() >= 0.30 && decal.halfSize() <= 0.75,
                    "strength=" + strength + " 时撞击环半径越界：" + decal.halfSize());
            }
        }
    }

    /** 全 11 招统一体检：没有 NaN / 非正 lifetime / 越界 alpha 能溜进渲染层。 */
    @Test
    void everyDeclaredSkillEmitsWellFormedSpawns() {
        int checked = 0;
        for (Identifier id : ZhenmaiPulsePlayer.EVENT_IDS) {
            checked += assertWellFormed(ZhenmaiPulsePlayer.plan(payload(id)), id);
        }
        for (Identifier id : BurstMeridianFamilyPlayer.EVENT_IDS) {
            checked += assertWellFormed(BurstMeridianFamilyPlayer.plan(payload(id)), id);
        }
        for (Identifier id : NpcSkillAuraPlayer.EVENT_IDS) {
            checked += assertWellFormed(NpcSkillAuraPlayer.plan(payload(id)), id);
        }
        assertTrue(checked > 100,
            "11 招合计应发出上百颗粒子，实际只有 " + checked + " —— 多半有招静默不出粒子");
    }

    private static int assertWellFormed(List<SkillParticleSpawn> spawns, Identifier id) {
        assertFalse(spawns.isEmpty(), id + " 一颗粒子都没发——该招在旁观者眼里完全没有视觉");
        for (SkillParticleSpawn spawn : spawns) {
            assertAllFinite(spawn, id.toString());
            assertTrue(spawn.maxAge() > 0, id + " 的粒子 lifetime 非正：" + spawn.maxAge());
            assertTrue(spawn.alpha() > 0.0f && spawn.alpha() <= 1.0f,
                id + " 的粒子 alpha 越界：" + spawn.alpha());
            assertTrue((spawn.rgb() & 0xFF000000) == 0,
                id + " 的颜色带了 alpha 通道字节，应是纯 0xRRGGBB：" + Integer.toHexString(spawn.rgb()));
        }
        return spawns.size();
    }

    private static void assertAllFinite(SkillParticleSpawn spawn, String context) {
        for (double value : new double[] {
            spawn.x(), spawn.y(), spawn.z(),
            spawn.velocityX(), spawn.velocityY(), spawn.velocityZ(),
        }) {
            assertTrue(Double.isFinite(value),
                context + " 产出了非有限坐标/速度（" + value + "）——粒子会静默消失或渲染异常");
        }
    }

    // ─── payload 构造helper ─────────────────────────────────────────────────────

    private static VfxEventPayload.SpawnParticle payload(Identifier eventId) {
        return payload(eventId, Optional.empty(), OptionalInt.empty(), Optional.empty(),
            OptionalInt.empty(), OptionalInt.empty());
    }

    private static VfxEventPayload.SpawnParticle countPayload(Identifier eventId, int count) {
        return payload(eventId, Optional.empty(), OptionalInt.empty(), Optional.empty(),
            OptionalInt.of(count), OptionalInt.empty());
    }

    private static VfxEventPayload.SpawnParticle payload(
        Identifier eventId,
        Optional<double[]> direction,
        OptionalInt colorRgb,
        Optional<Double> strength,
        OptionalInt count,
        OptionalInt durationTicks
    ) {
        return new VfxEventPayload.SpawnParticle(
            eventId, ORIGIN.clone(), direction, colorRgb, strength, count, durationTicks);
    }
}
