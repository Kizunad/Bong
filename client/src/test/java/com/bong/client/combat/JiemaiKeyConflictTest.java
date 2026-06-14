package com.bong.client.combat;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * interaction-intent-cleanup-v1 P3 — 截脉键位冲突回归。
 *
 * <p>截脉反应键此前默认 {@code GLFW_KEY_V}，与 {@code MovementKeybindings} 的冲刺键
 * （同样默认 V）冲突：单次按 V 两个 {@link net.minecraft.client.option.KeyBinding}
 * 的 {@code wasPressed()} 都返回 true → 冲刺的同时企图发截脉。修复后截脉默认未绑定，
 * 全仓只剩冲刺一处默认 V。本测试源码扫描锁住该不变量，防止任何一方改回 V 重新撞键。
 *
 * <p>采用源码扫描（同 {@code NoDuplicateDefaultGKeybindingTest}）而非实例化
 * {@code KeyBinding}——后者需 Minecraft bootstrap，单测环境不可用。
 */
public class JiemaiKeyConflictTest {
    private static final String CLIENT_MAIN = "src/main/java/com/bong/client";
    private static final String DASH_KEYBINDINGS =
        "src/main/java/com/bong/client/movement/MovementKeybindings.java";
    private static final String COMBAT_KEYBINDINGS =
        "src/main/java/com/bong/client/combat/CombatKeybindings.java";

    @Test
    void onlyMovementDashDefaultsToV() throws IOException {
        Path root = Path.of(CLIENT_MAIN);
        long occurrences;
        try (var files = Files.walk(root)) {
            occurrences = files
                .filter(path -> path.toString().endsWith(".java"))
                .map(JiemaiKeyConflictTest::read)
                .filter(text -> containsDefaultV(text))
                .count();
        }

        assertEquals(1, occurrences,
            "期望全仓只有 1 个 keybinding 默认绑定 V（移动冲刺），实际有 " + occurrences
                + " 个 —— 任意第二处默认 V 都会与冲刺撞键，单按一次 V 触发两个动作");
    }

    @Test
    void combatKeybindingsNoLongerDefaultsJiemaiToV() throws IOException {
        String combat = Files.readString(Path.of(COMBAT_KEYBINDINGS));
        assertFalse(containsDefaultV(combat),
            "期望 CombatKeybindings 不再把截脉键默认绑到 V（已改 GLFW_KEY_UNKNOWN），"
                + "实际仍含 GLFW_KEY_V —— V 键冲突回归");
        assertTrue(combat.contains("jiemai_react") && combat.contains("GLFW.GLFW_KEY_UNKNOWN"),
            "期望截脉反应键仍注册且默认 UNKNOWN（玩家显式绑定），实际未找到对应注册");
    }

    @Test
    void movementDashStillDefaultsToV() throws IOException {
        String movement = Files.readString(Path.of(DASH_KEYBINDINGS));
        assertTrue(containsDefaultV(movement),
            "期望冲刺键仍默认 V（本 plan 不动 movement，只挪截脉），实际未找到 GLFW_KEY_V");
    }

    private static boolean containsDefaultV(String source) {
        // 匹配 GLFW_KEY_V 但排除 GLFW_KEY_V0..V9 之类（当前无此常数，仍稳妥）。
        int idx = source.indexOf("GLFW.GLFW_KEY_V");
        while (idx >= 0) {
            int after = idx + "GLFW.GLFW_KEY_V".length();
            char next = after < source.length() ? source.charAt(after) : ' ';
            boolean wordContinues = Character.isLetterOrDigit(next) || next == '_';
            if (!wordContinues) {
                return true;
            }
            idx = source.indexOf("GLFW.GLFW_KEY_V", after);
        }
        return false;
    }

    private static String read(Path path) {
        try {
            return Files.readString(path);
        } catch (IOException exception) {
            throw new IllegalStateException(exception);
        }
    }
}
