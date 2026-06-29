package com.bong.client.inventory;

import com.bong.client.hud.SwordBondHudState;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * F.3 验证：InspectScreen.swordBondInfoLines() 灵剑区块渲染逻辑（active 二态）。
 *
 * <p>契约（sword-path-complete §F.3）：仅当 state.active()=true 显示三行：
 * 品阶 / 封存 / 人剑合一；active=false 不产生任何行（不渲染区块）。
 *
 * <p>使用纯静态方法测试，无需启动 MinecraftClient，确定可测。
 */
class InspectScreenSwordBondDisplayTest {

    // ─── active=true：产生三行 ────────────────────────────────────────────

    @Test
    void activeSwordBondShowsThreeLines() {
        SwordBondHudState state = new SwordBondHudState(true, 4, "三阶·凝", 0.75f, 0.8f, false);
        List<String> lines = InspectScreen.swordBondInfoLines(state);
        assertEquals(3, lines.size(),
            "active=true 应产生 3 行灵剑信息（品阶 / 封存 / 人剑合一）；实际=" + lines.size());
    }

    @Test
    void activeSwordBondFirstLineContainsGradeName() {
        SwordBondHudState state = new SwordBondHudState(true, 4, "三阶·凝", 0.75f, 0.8f, false);
        List<String> lines = InspectScreen.swordBondInfoLines(state);
        assertTrue(lines.get(0).contains("三阶·凝"),
            "品阶行（第 0 行）应含 gradeName '三阶·凝'；"
                + "实际第 0 行=" + lines.get(0));
    }

    @Test
    void activeSwordBondSecondLineContainsStoredRatioPercent() {
        // storedQiRatio=0.75 → 显示 "75"（百分比）
        SwordBondHudState state = new SwordBondHudState(true, 4, "三阶·凝", 0.75f, 0.8f, false);
        List<String> lines = InspectScreen.swordBondInfoLines(state);
        assertTrue(lines.get(1).contains("75"),
            "封存行（第 1 行）应含 storedQiRatio 百分比 '75'（0.75×100）；"
                + "实际第 1 行=" + lines.get(1));
    }

    @Test
    void activeSwordBondThirdLineContainsBondStrengthPercent() {
        // bondStrength=0.8 → 显示 "80"（百分比）
        SwordBondHudState state = new SwordBondHudState(true, 4, "三阶·凝", 0.75f, 0.8f, false);
        List<String> lines = InspectScreen.swordBondInfoLines(state);
        assertTrue(lines.get(2).contains("80"),
            "人剑合一行（第 2 行）应含 bondStrength 百分比 '80'（0.8×100）；"
                + "实际第 2 行=" + lines.get(2));
    }

    // ─── active=false：产生零行 ───────────────────────────────────────────

    @Test
    void inactiveSwordBondShowsNoLines() {
        List<String> lines = InspectScreen.swordBondInfoLines(SwordBondHudState.INACTIVE);
        assertTrue(lines.isEmpty(),
            "active=false（INACTIVE 哨兵）不应产生任何灵剑信息行（不渲染区块，防留白影响布局）；"
                + "实际=" + lines.size() + " 行");
    }

    @Test
    void explicitlyInactiveSwordBondShowsNoLines() {
        SwordBondHudState inactive = new SwordBondHudState(false, 3, "凝脉", 0.5f, 0.6f, false);
        List<String> lines = InspectScreen.swordBondInfoLines(inactive);
        assertTrue(lines.isEmpty(),
            "active=false（即使携带有效数值）不应产生行；"
                + "实际=" + lines.size() + " 行，内容=" + lines);
    }

    // ─── 边界值：零值 & 满值 ──────────────────────────────────────────────

    @Test
    void activeSwordBondWithZeroValuesStillShowsThreeLines() {
        SwordBondHudState state = new SwordBondHudState(true, 0, "", 0f, 0f, false);
        List<String> lines = InspectScreen.swordBondInfoLines(state);
        assertEquals(3, lines.size(),
            "active=true 即使 storedQiRatio=0 bondStrength=0 gradeName='' 也应产生 3 行；"
                + "实际=" + lines.size());
    }

    @Test
    void activeSwordBondWithMaxValuesShowsHundredPercent() {
        SwordBondHudState state = new SwordBondHudState(true, 6, "化虚", 1.0f, 1.0f, true);
        List<String> lines = InspectScreen.swordBondInfoLines(state);
        assertEquals(3, lines.size(), "满值时应有 3 行；实际=" + lines.size());
        assertTrue(lines.get(1).contains("100"),
            "满封存（storedQiRatio=1.0）封存行应含 '100'；实际=" + lines.get(1));
        assertTrue(lines.get(2).contains("100"),
            "满人剑合一（bondStrength=1.0）行应含 '100'；实际=" + lines.get(2));
    }

    @Test
    void activeSwordBondWithClampedRatioShowsHundredPercent() {
        // SwordBondHudState 内部 clamp(storedQiRatio, 0, 1)；传 2.0 实际存 1.0，传 -0.5 实际存 0.0
        SwordBondHudState state = new SwordBondHudState(true, 4, "固元", 2.0f, -0.5f, false);
        List<String> lines = InspectScreen.swordBondInfoLines(state);
        assertEquals(3, lines.size(), "clamp 后 active=true 应仍有 3 行；实际=" + lines.size());
        // storedQiRatio clamp=1.0 → "...100%"
        assertTrue(lines.get(1).contains("100"),
            "storedQiRatio=2.0 clamp 到 1.0 后封存行应含 '100'；实际=" + lines.get(1));
        // bondStrength clamp=0.0 → "...§f0%" （用 "§f0%" 精确匹配，避免与 "100" 中的 "0" 混淆）
        assertTrue(lines.get(2).contains("§f0%"),
            "bondStrength=-0.5 clamp 到 0.0 后人剑合一行应含 '§f0%'；实际=" + lines.get(2));
    }

    // ─── 行结构：含 gradeName 关键字 "品阶" / "封存" / "人剑合一" ──────────

    @Test
    void activeSwordBondLineLabelsPresent() {
        SwordBondHudState state = new SwordBondHudState(true, 5, "通灵", 0.5f, 0.7f, false);
        List<String> lines = InspectScreen.swordBondInfoLines(state);
        assertEquals(3, lines.size());
        assertTrue(lines.get(0).contains("品阶"),
            "第 0 行应含 '品阶' 标签；实际=" + lines.get(0));
        assertTrue(lines.get(1).contains("封存"),
            "第 1 行应含 '封存' 标签；实际=" + lines.get(1));
        assertTrue(lines.get(2).contains("人剑合一"),
            "第 2 行应含 '人剑合一' 标签；实际=" + lines.get(2));
    }
}
