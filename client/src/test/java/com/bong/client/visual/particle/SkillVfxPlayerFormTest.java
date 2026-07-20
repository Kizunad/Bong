package com.bong.client.visual.particle;

import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.util.EnumSet;
import java.util.HashSet;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-skill-anim-fidelity-v1 P5 —— 三个新 VfxPlayer 的**形态分支逐条覆盖**。
 *
 * <p>{@code play()} 本身要 {@code MinecraftClient} + {@code ClientWorld}，纯 JVM 单测里跑不了；
 * 但真正会漂移的东西——「哪个 event_id 走哪条形态分支」「每条分支的 spec 参数」「方向向量退化时
 * 怎么兜底」——都被抽成了静态纯函数（{@code formFor} / {@code normalizedDirection} /
 * {@code perpendicular}）与枚举常量，这里逐条锁死。
 *
 * <p>与 {@link SkillVfxWiringManifestTest} 的分工：那边锁「server 发的 id 能被注册表接住并落到
 * 声明的类」，本文件锁「进了那个类之后，走对分支、参数没漂」。两者都不可省——接对了类但内部
 * 分支串了（比如 harden 走成 parry 的形态），注册表层面完全看不出来。
 */
public class SkillVfxPlayerFormTest {

    // ─── ZhenmaiPulsePlayer：5 招形态 ────────────────────────────────────────────

    @Test
    void zhenmaiFormForResolvesEveryDeclaredEventId() {
        assertSame(ZhenmaiPulsePlayer.Form.PARRY,
            ZhenmaiPulsePlayer.formFor(ZhenmaiPulsePlayer.PARRY_FLASH));
        assertSame(ZhenmaiPulsePlayer.Form.NEUTRALIZE,
            ZhenmaiPulsePlayer.formFor(ZhenmaiPulsePlayer.NEUTRALIZE_DUST));
        assertSame(ZhenmaiPulsePlayer.Form.MULTIPOINT,
            ZhenmaiPulsePlayer.formFor(ZhenmaiPulsePlayer.MULTIPOINT_RING));
        assertSame(ZhenmaiPulsePlayer.Form.HARDEN,
            ZhenmaiPulsePlayer.formFor(ZhenmaiPulsePlayer.HARDEN_SHELL));
        assertSame(ZhenmaiPulsePlayer.Form.SEVER,
            ZhenmaiPulsePlayer.formFor(ZhenmaiPulsePlayer.SEVER_SNAP));
    }

    /** 每个 Form 变体都必须能被自己的 eventId 反查到（枚举加了新招但 EVENT_IDS 忘同步会撞红）。 */
    @Test
    void zhenmaiEveryFormVariantIsReachableAndExported() {
        Set<Identifier> exported = new HashSet<>();
        for (Identifier id : ZhenmaiPulsePlayer.EVENT_IDS) {
            exported.add(id);
        }
        for (ZhenmaiPulsePlayer.Form form : ZhenmaiPulsePlayer.Form.values()) {
            assertSame(form, ZhenmaiPulsePlayer.formFor(form.eventId),
                "Form." + form + " 的 eventId 反查不到自己");
            assertTrue(exported.contains(form.eventId),
                "Form." + form + " 的 eventId 未出现在 EVENT_IDS——VfxBootstrap 不会注册它，"
                    + "server 发了也是 bridgeMiss");
        }
        assertEquals(ZhenmaiPulsePlayer.Form.values().length, exported.size(),
            "EVENT_IDS 与 Form 变体数量不一致");
    }

    @Test
    void zhenmaiFormForRejectsUnknownAndNull() {
        assertNull(ZhenmaiPulsePlayer.formFor(new Identifier("bong", "not_a_zhenmai_skill")),
            "非真脉 id 必须返回 null——注册表把别家 id 派进来时要静默跳过而不是画错东西");
        assertNull(ZhenmaiPulsePlayer.formFor(new Identifier("minecraft", "flame")),
            "非 bong 命名空间的 id 同样返回 null");
        assertNull(ZhenmaiPulsePlayer.formFor(null), "null id 必须返回 null 而不是 NPE");
    }

    /**
     * 金脉明度阶梯：harden < neutralize < parry < multipoint < sever，与招式烈度同序。
     * plan §P5.1 ① 把这条写成读招前提，所以逐对锁而不是只锁"互不相同"。
     */
    @Test
    void zhenmaiFallbackColorsFormMonotonicBrightnessLadder() {
        int harden = ZhenmaiPulsePlayer.Form.HARDEN.fallbackRgb;
        int neutralize = ZhenmaiPulsePlayer.Form.NEUTRALIZE.fallbackRgb;
        int parry = ZhenmaiPulsePlayer.Form.PARRY.fallbackRgb;
        int multipoint = ZhenmaiPulsePlayer.Form.MULTIPOINT.fallbackRgb;
        int sever = ZhenmaiPulsePlayer.Form.SEVER.fallbackRgb;

        assertTrue(luminance(harden) < luminance(neutralize),
            "harden 应比 neutralize 沉：" + hex(harden) + " vs " + hex(neutralize));
        assertTrue(luminance(neutralize) < luminance(parry),
            "neutralize 应比 parry 沉：" + hex(neutralize) + " vs " + hex(parry));
        assertTrue(luminance(parry) < luminance(multipoint),
            "parry 应比 multipoint 沉：" + hex(parry) + " vs " + hex(multipoint));
        assertTrue(luminance(multipoint) < luminance(sever),
            "multipoint 应比 sever 沉：" + hex(multipoint) + " vs " + hex(sever));

        assertEquals(0xD4AF6A, parry,
            "parry 应持金脉 anchor #D4AF6A（plan §P5.1 ① 字面指定）");
    }

    /** 每招的形态参数必须真的不同——否则 5 个 id 画出同一个东西，等于没去复用。 */
    @Test
    void zhenmaiFormsHaveDistinctMotionOrGeometry() {
        // 运动形态至少覆盖 3 种（5 招里 parry/sever 同为 FORWARD 但速度与穴位点数不同）。
        Set<ZhenmaiPulsePlayer.Form.Motion> motions =
            EnumSet.noneOf(ZhenmaiPulsePlayer.Form.Motion.class);
        for (ZhenmaiPulsePlayer.Form form : ZhenmaiPulsePlayer.Form.values()) {
            motions.add(form.motion);
        }
        assertEquals(4, motions.size(),
            "真脉 5 招应覆盖全部 4 种运动形态，实际 " + motions);

        // 同为 FORWARD 的 parry / sever 必须在速度与穴位点数上拉开。
        assertEquals(ZhenmaiPulsePlayer.Form.Motion.FORWARD, ZhenmaiPulsePlayer.Form.PARRY.motion);
        assertEquals(ZhenmaiPulsePlayer.Form.Motion.FORWARD, ZhenmaiPulsePlayer.Form.SEVER.motion);
        assertTrue(ZhenmaiPulsePlayer.Form.SEVER.speed > ZhenmaiPulsePlayer.Form.PARRY.speed,
            "断脉爆冲应快于弹反闪，否则两招同为前向形态时无从分辨");
        assertTrue(
            ZhenmaiPulsePlayer.Form.SEVER.acupoints < ZhenmaiPulsePlayer.Form.PARRY.acupoints,
            "断脉是爆点（穴位点少），弹反是护势（穴位点多）");

        // 所有形态的数量参数必须为正，否则该招静默不出粒子。
        for (ZhenmaiPulsePlayer.Form form : ZhenmaiPulsePlayer.Form.values()) {
            assertTrue(form.defaultPulses > 0, form + " 的默认脉冲数必须为正");
            assertTrue(form.acupoints > 0, form + " 的穴位点数必须为正");
            assertTrue(form.radius > 0.0, form + " 的铺开半径必须为正");
            assertTrue(form.speed > 0.0, form + " 的运动速度必须为正");
        }
    }

    // ─── 方向向量退化兜底（三个 player 共用，血崩步也调它）────────────────────────

    @Test
    void normalizedDirectionFallsBackOnDegenerateInput() {
        double[] fallback = { 1.0, 0.0, 0.0 };
        assertArrayEquals(fallback, ZhenmaiPulsePlayer.normalizedDirection(null), 1e-9,
            "缺 direction 时应回退 +X");
        assertArrayEquals(fallback,
            ZhenmaiPulsePlayer.normalizedDirection(new double[] { 0.0, 0.0, 0.0 }), 1e-9,
            "零向量无法归一，应回退 +X 而不是产生 NaN 坐标（NaN 会让粒子消失或渲染异常）");
        assertArrayEquals(fallback,
            ZhenmaiPulsePlayer.normalizedDirection(new double[] { Double.NaN, 0.0, 0.0 }), 1e-9,
            "NaN 分量应回退 +X");
        assertArrayEquals(fallback,
            ZhenmaiPulsePlayer.normalizedDirection(
                new double[] { Double.POSITIVE_INFINITY, 0.0, 0.0 }), 1e-9,
            "无穷大分量应回退 +X");
        assertArrayEquals(fallback, ZhenmaiPulsePlayer.normalizedDirection(new double[] { 1.0 }),
            1e-9, "长度非 3 的数组应回退 +X");
    }

    @Test
    void normalizedDirectionNormalizesRegularVectors() {
        double[] result = ZhenmaiPulsePlayer.normalizedDirection(new double[] { 0.0, 0.0, 5.0 });
        assertArrayEquals(new double[] { 0.0, 0.0, 1.0 }, result, 1e-9);

        double[] diagonal = ZhenmaiPulsePlayer.normalizedDirection(new double[] { 3.0, 0.0, 4.0 });
        assertEquals(1.0, length(diagonal), 1e-9, "归一化后模长应为 1");
        assertEquals(0.6, diagonal[0], 1e-9);
        assertEquals(0.8, diagonal[2], 1e-9);
    }

    @Test
    void perpendicularIsUnitAndOrthogonalIncludingVerticalFallback() {
        double[] horizontal = { 1.0, 0.0, 0.0 };
        double[] side = ZhenmaiPulsePlayer.perpendicular(horizontal);
        assertEquals(1.0, length(side), 1e-9, "侧轴应为单位向量");
        assertEquals(0.0, dot(horizontal, side), 1e-9, "侧轴应与 direction 正交");

        // 竖直 direction 时叉积退化，必须走 +Z 兜底而不是返回零向量（零向量会让扇面塌成一点）。
        double[] vertical = { 0.0, 1.0, 0.0 };
        double[] verticalSide = ZhenmaiPulsePlayer.perpendicular(vertical);
        assertEquals(1.0, length(verticalSide), 1e-9,
            "竖直方向下侧轴仍须是单位向量（退化返回零向量会让脉冲扇面塌成一点）");
    }

    // ─── BurstMeridianFamilyPlayer：3 招形态 ─────────────────────────────────────

    @Test
    void burstFormForResolvesEveryDeclaredEventId() {
        assertSame(BurstMeridianFamilyPlayer.Form.IMPACT_RING,
            BurstMeridianFamilyPlayer.formFor(BurstMeridianFamilyPlayer.TIE_SHAN_KAO));
        assertSame(BurstMeridianFamilyPlayer.Form.DASH_AFTERIMAGE,
            BurstMeridianFamilyPlayer.formFor(BurstMeridianFamilyPlayer.XUE_BENG_BU));
        assertSame(BurstMeridianFamilyPlayer.Form.BODY_COUNTERFLOW,
            BurstMeridianFamilyPlayer.formFor(BurstMeridianFamilyPlayer.NI_MAI_HU_TI));
    }

    @Test
    void burstFormForRejectsUnknownNullAndBengQuanItself() {
        assertNull(BurstMeridianFamilyPlayer.formFor(null), "null id 必须返回 null 而不是 NPE");
        assertNull(BurstMeridianFamilyPlayer.formFor(new Identifier("bong", "nope")));
        // 崩拳本尊归 BurstMeridianBengQuanPlayer，本类不得接管（接管会让崩拳改画成靠撞冲击环）。
        assertNull(
            BurstMeridianFamilyPlayer.formFor(BurstMeridianBengQuanPlayer.EVENT_ID),
            "崩拳本尊 " + BurstMeridianBengQuanPlayer.EVENT_ID
                + " 必须仍由 BurstMeridianBengQuanPlayer 承接，本类不接管");
    }

    @Test
    void burstEveryFormVariantIsReachableAndExported() {
        Set<Identifier> exported = new HashSet<>();
        for (Identifier id : BurstMeridianFamilyPlayer.EVENT_IDS) {
            exported.add(id);
        }
        for (BurstMeridianFamilyPlayer.Form form : BurstMeridianFamilyPlayer.Form.values()) {
            assertSame(form, BurstMeridianFamilyPlayer.formFor(form.eventId));
            assertTrue(exported.contains(form.eventId),
                "Form." + form + " 的 eventId 未出现在 EVENT_IDS");
            assertTrue(form.defaultCount > 0, form + " 的默认粒子数必须为正");
            assertTrue(form.defaultLifetime > 0, form + " 的默认 lifetime 必须为正");
        }
        assertEquals(BurstMeridianFamilyPlayer.Form.values().length, exported.size());
    }

    /** 爆脉是「共用色 + 形态分化」，所以家族色只有一个常量，不存在逐招配色。 */
    @Test
    void burstFamilyHasSingleSharedIdentityColor() {
        assertEquals(0xC58B3F, BurstMeridianFamilyPlayer.FAMILY_RGB,
            "爆脉家族识别色被改动——plan §P5.1 ② 指定 #C58B3F");
    }

    // 护体环 lifetime（12t = 一个重发间隔，而非 60t 护体窗口）的契约锁**不在本文件**，
    // 刻意不在这里抄第三份 60/12：
    //   · client ↔ plan：SkillParticleSpecDocTest 解析 plan §P5.1 ② 表逐格对拍 defaultLifetime；
    //   · 窗口铺满算术：server burst_meridian 的
    //     p5_ni_mai_hu_ti_aura_cadence_tiles_buff_window_exactly（常量在那边原生持有）。
    // 单环若撑满整个 buff 窗口就会钉死在施法瞬间的世界坐标上（SpawnParticle payload 无实体
    // 标识），玩家一移动纹即脱离体表——窗口由 server 逐环重发铺满，client 只画好单个环。

    // ─── NpcSkillAuraPlayer：3 招形态 ────────────────────────────────────────────

    @Test
    void npcFormForResolvesEveryDeclaredEventId() {
        assertSame(NpcSkillAuraPlayer.Form.HEAL_COLUMN,
            NpcSkillAuraPlayer.formFor(NpcSkillAuraPlayer.HEAL_BASIC));
        assertSame(NpcSkillAuraPlayer.Form.SPEED_DUST,
            NpcSkillAuraPlayer.formFor(NpcSkillAuraPlayer.BUFF_SPEED));
        assertSame(NpcSkillAuraPlayer.Form.DEFENSE_RING,
            NpcSkillAuraPlayer.formFor(NpcSkillAuraPlayer.BUFF_DEFENSE));
    }

    @Test
    void npcFormForRejectsUnknownAndNull() {
        assertNull(NpcSkillAuraPlayer.formFor(null), "null id 必须返回 null 而不是 NPE");
        assertNull(NpcSkillAuraPlayer.formFor(new Identifier("bong", "npc_not_a_skill")));
        // 去复用前借的那三个 id 不得再被本类接住（接住等于借用换了个方向复活）。
        for (String legacy : new String[] {
            "yidao_meridian_repair", "jiemai_neutralize_dust", "burst_meridian_beng_quan"
        }) {
            assertNull(NpcSkillAuraPlayer.formFor(new Identifier("bong", legacy)),
                "NPC player 不得接管 P5 之前借用的 bong:" + legacy);
        }
    }

    @Test
    void npcEveryFormVariantIsReachableAndExported() {
        Set<Identifier> exported = new HashSet<>();
        for (Identifier id : NpcSkillAuraPlayer.EVENT_IDS) {
            exported.add(id);
        }
        for (NpcSkillAuraPlayer.Form form : NpcSkillAuraPlayer.Form.values()) {
            assertSame(form, NpcSkillAuraPlayer.formFor(form.eventId));
            assertTrue(exported.contains(form.eventId),
                "Form." + form + " 的 eventId 未出现在 EVENT_IDS");
        }
        assertEquals(NpcSkillAuraPlayer.Form.values().length, exported.size());
    }

    /**
     * NPC 三招配色必须真的拉得开——plan §P5.1 ③ 的「颜色必须独立」不是「hex 不相等」，
     * 旧值 #9FD8C8 vs #A8E6CF 就不相等却远距离不可辨。这里用通道曼哈顿距离做判据，
     * 与 server 侧 `npc_skills_have_independent_ids_and_widely_separated_colors` 同口径。
     */
    @Test
    void npcFallbackColorsAreWidelySeparated() {
        int[] colors = {
            NpcSkillAuraPlayer.HEAL_RGB,
            NpcSkillAuraPlayer.SPEED_RGB,
            NpcSkillAuraPlayer.DEFENSE_RGB,
        };
        for (int i = 0; i < colors.length; i++) {
            for (int j = i + 1; j < colors.length; j++) {
                int distance = channelDistance(colors[i], colors[j]);
                assertTrue(distance >= 0x60,
                    "NPC 配色 " + hex(colors[i]) + " 与 " + hex(colors[j]) + " 距离仅 " + distance
                        + "（要求 ≥ 96）——两招远距离不可辨");
            }
        }
        // 明确锁住"旧的淡青绿"不再出现（回退到它就重新踩进不可辨的坑）。
        for (int color : colors) {
            assertTrue(color != 0x9FD8C8,
                "NPC 配色回退到了旧值 #9FD8C8——它与 heal #A8E6CF 同属淡青绿、远距离不可辨");
        }
    }

    /** 三个 player 的 EVENT_IDS 两两不相交——同一个 id 被两个 player 声明必然有一个注册被覆盖。 */
    @Test
    void thePlayersClaimDisjointEventIdSets() {
        Set<Identifier> all = new HashSet<>();
        int total = 0;
        for (Identifier[] ids : new Identifier[][] {
            ZhenmaiPulsePlayer.EVENT_IDS,
            BurstMeridianFamilyPlayer.EVENT_IDS,
            NpcSkillAuraPlayer.EVENT_IDS,
        }) {
            for (Identifier id : ids) {
                assertNotNull(id);
                all.add(id);
                total++;
            }
        }
        assertEquals(total, all.size(),
            "三个 player 声明的 event_id 出现重叠——后注册的会静默覆盖先注册的");
        assertEquals(11, total, "P5 共 11 招接线（真脉 5 + 爆脉 3 + NPC 3）");
    }

    private static int channelDistance(int left, int right) {
        return Math.abs(((left >> 16) & 0xFF) - ((right >> 16) & 0xFF))
            + Math.abs(((left >> 8) & 0xFF) - ((right >> 8) & 0xFF))
            + Math.abs((left & 0xFF) - (right & 0xFF));
    }

    private static int luminance(int rgb) {
        // 简易感知亮度（BT.601 权重 ×1000 取整，避免浮点比较噪声）。
        return 299 * ((rgb >> 16) & 0xFF) + 587 * ((rgb >> 8) & 0xFF) + 114 * (rgb & 0xFF);
    }

    private static String hex(int rgb) {
        return String.format("#%06X", rgb);
    }

    private static double length(double[] v) {
        return Math.sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    }

    private static double dot(double[] a, double[] b) {
        return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    }
}
