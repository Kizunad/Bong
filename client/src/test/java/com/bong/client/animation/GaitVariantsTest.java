package com.bong.client.animation;

import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.EnumSet;
import java.util.List;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * {@link GaitVariants} 的饱和覆盖 + 变体资产的落地核验。
 *
 * <p>第二部分（资产存在、循环闭合、脚下与基准步态逐帧一致）才是这份测试的重点：
 * 变体表里写错一个 id 不会报错，只会**静默回落**到全局步态——玩家看到的是"携行手型
 * 没生效"，而不是任何异常。没有资产存在性核验，这种断链可以一直活着。
 */
class GaitVariantsTest {

    private static final String KNIFE = GaitVariants.HERB_KNIFE_IRON;

    // ── 纯函数：命中 ────────────────────────────────────────────────────

    @Test
    void knifeWalkResolvesToTheCarryVariant() {
        assertEquals(new Identifier("bong", "herb_knife_carry_walk"),
            GaitVariants.resolve(GaitSelector.Gait.WALK, KNIFE));
    }

    @Test
    void sprintDeliberatelyHasNoCarryVariant() {
        // 手稿的冲刺只泵肘、前后摆幅恒定，做成变体会把手臂钉死、盖掉 vanilla 的摆臂，
        // 比回落更差（见 gen_herb_knife_carry_gait 模块文档）。这条是**故意**没有。
        assertEquals(GaitSelector.Gait.SPRINT.animId(),
            GaitVariants.resolve(GaitSelector.Gait.SPRINT, KNIFE));
    }

    // ── 纯函数：回落 ────────────────────────────────────────────────────

    @Test
    void gaitsWithoutAVariantFallBackToTheGlobalGait() {
        // 目前只有 walk 有携行版；其余档回落是**安全降级**不是断链
        for (GaitSelector.Gait gait : EnumSet.of(
                GaitSelector.Gait.JOG, GaitSelector.Gait.SPRINT, GaitSelector.Gait.DASH)) {
            assertEquals(gait.animId(), GaitVariants.resolve(gait, KNIFE),
                gait + " 没有携行变体时应回落到全局步态");
        }
    }

    @Test
    void unknownTemplateFallsBackOnEveryGait() {
        for (GaitSelector.Gait gait : GaitSelector.Gait.values()) {
            assertEquals(gait.animId(), GaitVariants.resolve(gait, "bronze_saber"),
                gait + " 在未登记变体的手持物下应回落");
        }
    }

    @Test
    void emptyHandFallsBackOnEveryGait() {
        for (GaitSelector.Gait gait : GaitSelector.Gait.values()) {
            assertEquals(gait.animId(), GaitVariants.resolve(gait, null),
                gait + " 空手时应回落到全局步态");
        }
    }

    @Test
    void templateIdIsTrimmedBeforeLookup() {
        assertEquals(new Identifier("bong", "herb_knife_carry_walk"),
            GaitVariants.resolve(GaitSelector.Gait.WALK, "  " + KNIFE + "  "),
            "template_id 两侧空白不该让变体查不到");
    }

    // ── 纯函数：边界 ────────────────────────────────────────────────────

    @Test
    void noneGaitResolvesToNullWhateverIsHeld() {
        assertNull(GaitVariants.resolve(GaitSelector.Gait.NONE, KNIFE));
        assertNull(GaitVariants.resolve(GaitSelector.Gait.NONE, null));
        assertNull(GaitVariants.resolve(GaitSelector.Gait.NONE, "bronze_saber"));
    }

    @Test
    void nullGaitResolvesToNull() {
        assertNull(GaitVariants.resolve(null, KNIFE));
        assertNull(GaitVariants.resolve(null, null));
    }

    @Test
    void hasVariantsAnswersForBothSides() {
        assertTrue(GaitVariants.hasVariants(KNIFE));
        assertTrue(GaitVariants.hasVariants(" " + KNIFE + " "));
        assertFalse(GaitVariants.hasVariants("bronze_saber"));
        assertFalse(GaitVariants.hasVariants(null));
    }

    @Test
    void everyGaitResolvesToSomethingPlayableOrNull() {
        // 状态机全覆盖：每个档位 × 有/无变体的手持物，都不许抛
        for (GaitSelector.Gait gait : GaitSelector.Gait.values()) {
            for (String held : new String[] {KNIFE, "bronze_saber", null, "", "   "}) {
                Identifier resolved = GaitVariants.resolve(gait, held);
                if (gait.animId() == null) {
                    assertNull(resolved, gait + " 无动画档应恒为 null");
                } else {
                    assertNotNull(resolved, gait + " + " + held + " 解析出了 null");
                }
            }
        }
    }

    // ── 资产：id 必须真有对应的动画文件 ──────────────────────────────────

    @Test
    void everyVariantIdHasARealAnimationAsset() throws IOException {
        Path root = animationAssetRoot();
        for (GaitSelector.Gait gait : GaitSelector.Gait.values()) {
            Identifier id = GaitVariants.resolve(gait, KNIFE);
            if (id == null || id.equals(gait.animId())) {
                continue;                       // 回落到全局步态的档不在本条覆盖内
            }
            Path asset = root.resolve(id.getPath() + ".json");
            assertTrue(Files.isRegularFile(asset),
                "变体 " + id + " 在 " + asset + " 没有对应资产——查不到会静默回落，"
                    + "玩家只会看到'携行手型没生效'，不会有任何报错");
        }
    }

    @Test
    void carryVariantsLoopAndCloseTheLoop() throws IOException {
        // 循环动画每个用到的 axis 必须首末同值，否则 findAfter 会 fabricate 一个
        // (endTick+1, defaultValue) 虚拟帧，把整条循环拖回 0（conventions §7.1）
        Path root = animationAssetRoot();
        for (String path : List.of("herb_knife_carry_walk")) {
            String json = Files.readString(root.resolve(path + ".json"));
            assertTrue(json.contains("\"isLoop\": true"), path + " 必须是循环动画");
            assertEquals(0, loopBoundaryMismatches(json, path).size(),
                path + " 首末不等的轴：" + loopBoundaryMismatches(json, path));
        }
    }

    @Test
    void carryVariantsNeverTouchTorsoOrHead() throws IOException {
        // 携行层有意放宽到"可以写手臂"，但 torso/head 必须留给"边走边看四周"与招式的
        // 躯干拧转——写了就会把玩家的视线方向按在动画值上
        Path root = animationAssetRoot();
        for (String path : List.of("herb_knife_carry_walk")) {
            String json = Files.readString(root.resolve(path + ".json"));
            assertFalse(json.contains("\"torso\""), path + " 写了 torso");
            assertFalse(json.contains("\"head\""), path + " 写了 head");
        }
    }

    /** 极简 JSON 扫描：收集 tick 0 与 endTick 上取值不同的 (part, axis)。 */
    private static Set<String> loopBoundaryMismatches(String json, String name) {
        java.util.regex.Matcher endM =
            java.util.regex.Pattern.compile("\"endTick\"\\s*:\\s*(\\d+)").matcher(json);
        assertTrue(endM.find(), name + " 没有 endTick");
        String end = endM.group(1);
        java.util.Map<String, String> first = new java.util.HashMap<>();
        java.util.Map<String, String> last = new java.util.HashMap<>();
        java.util.regex.Matcher m = java.util.regex.Pattern.compile(
            "\\{\\s*\"tick\"\\s*:\\s*(\\d+)\\s*,\\s*\"easing\"\\s*:\\s*\"[^\"]*\"\\s*,\\s*"
                + "\"(\\w+)\"\\s*:\\s*\\{\\s*\"(\\w+)\"\\s*:\\s*(-?[\\d.eE+-]+)").matcher(json);
        while (m.find()) {
            String key = m.group(2) + "." + m.group(3);
            if ("0".equals(m.group(1))) {
                first.put(key, m.group(4));
            } else if (end.equals(m.group(1))) {
                last.put(key, m.group(4));
            }
        }
        assertFalse(first.isEmpty(), name + " tick 0 一条轨道都没解析到——正则失配了");
        Set<String> bad = new java.util.HashSet<>();
        for (var e : first.entrySet()) {
            if (!e.getValue().equals(last.get(e.getKey()))) {
                bad.add(e.getKey() + "(t0=" + e.getValue() + ", t" + end + "=" + last.get(e.getKey()) + ")");
            }
        }
        return bad;
    }

    private static Path animationAssetRoot() {
        Path cwd = Path.of(System.getProperty("user.dir"));
        for (Path candidate : List.of(
                cwd.resolve("src/main/resources/assets/bong/player_animation"),
                cwd.resolve("client/src/main/resources/assets/bong/player_animation"))) {
            if (Files.isDirectory(candidate)) {
                return candidate;
            }
        }
        throw new IllegalStateException("无法定位 player_animation 根（user.dir=" + cwd + "）");
    }
}
