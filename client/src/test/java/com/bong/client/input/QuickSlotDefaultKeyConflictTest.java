package com.bong.client.input;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-bughunt-quick-slot-function-key-collision-v1 — F1-F9 默认键保留区回归。
 *
 * <p>快捷使用栏把九个正式入口固定展示为 F1-F9。其它独立 tick 消费链如果再次默认
 * 占用同一功能键，注册顺序最多只能决定哪一侧抢到按键，无法保证两个入口都可达。
 * 测试采用源码契约扫描，避免在普通 JUnit 环境启动 Minecraft client bootstrap。
 */
public class QuickSlotDefaultKeyConflictTest {
    private static final Path CLIENT_MAIN = Path.of("src/main/java/com/bong/client");
    private static final Path COMBAT_KEYBINDINGS = CLIENT_MAIN.resolve("combat/CombatKeybindings.java");
    private static final Path QUICK_SLOT_CONFIG = CLIENT_MAIN.resolve("combat/QuickSlotConfig.java");
    private static final Path HUD_IMMERSION = CLIENT_MAIN.resolve("hud/HudImmersionControls.java");
    private static final Path NPC_INTERACTION_LOG = CLIENT_MAIN.resolve("npc/NpcInteractionLogControls.java");
    private static final Pattern DIRECT_FUNCTION_KEY =
        Pattern.compile("GLFW\\.GLFW_KEY_F([1-9])(?![0-9A-Z_])");

    @Test
    void quickSlotsStillOwnFunctionKeyDefaults() throws IOException {
        String combat = Files.readString(COMBAT_KEYBINDINGS);
        String config = Files.readString(QUICK_SLOT_CONFIG);

        assertTrue(config.contains("public static final int SLOT_COUNT = 9"),
            "期望快捷使用栏继续保留 9 个槽位，对应 F1-F9；SLOT_COUNT 已漂移");
        assertTrue(combat.contains("GLFW.GLFW_KEY_F1 + i"),
            "期望 CombatKeybindings 继续按 F1+i 注册九个快捷槽默认键");
    }

    @Test
    void hudImmersionDefaultsUnbound() throws IOException {
        String source = Files.readString(HUD_IMMERSION);

        assertTrue(source.contains("key.bong-client.hud_immersive_toggle"),
            "HUD 沉浸控制必须继续注册到控制菜单，不能靠删除入口规避冲突");
        assertTrue(source.contains("GLFW.GLFW_KEY_UNKNOWN"),
            "HUD 沉浸开关应默认未绑定，由玩家在控制菜单显式分配");
        assertFalse(source.contains("GLFW.GLFW_KEY_F6"),
            "HUD 沉浸开关不得重新占用快捷槽 6 的默认 F6");
    }

    @Test
    void npcInteractionLogDefaultsUnbound() throws IOException {
        String source = Files.readString(NPC_INTERACTION_LOG);

        assertTrue(source.contains("key.bong-client.npc_interaction_log"),
            "NPC 交互日志必须继续注册到控制菜单，不能靠删除入口规避冲突");
        assertTrue(source.contains("GLFW.GLFW_KEY_UNKNOWN"),
            "NPC 交互日志应默认未绑定，由玩家在控制菜单显式分配");
        assertFalse(source.contains("GLFW.GLFW_KEY_F7"),
            "NPC 交互日志不得重新占用快捷槽 7 的默认 F7");
    }

    @Test
    void noOtherClientBindingClaimsF1ToF9ByDefault() throws IOException {
        List<String> collisions = new ArrayList<>();
        try (var files = Files.walk(CLIENT_MAIN)) {
            files.filter(path -> path.toString().endsWith(".java"))
                .filter(path -> !path.equals(COMBAT_KEYBINDINGS))
                .forEach(path -> collectFunctionKeyClaims(path, collisions));
        }

        assertEquals(List.of(), collisions,
            "F1-F9 是快捷使用槽默认键保留区；其它生产 keybinding 不得直接占用: "
                + collisions);
    }

    private static void collectFunctionKeyClaims(Path path, List<String> collisions) {
        String source = read(path);
        if (!source.contains("new KeyBinding")) {
            return;
        }
        Matcher matcher = DIRECT_FUNCTION_KEY.matcher(source);
        while (matcher.find()) {
            collisions.add(CLIENT_MAIN.relativize(path) + ":" + matcher.group());
        }
    }

    private static String read(Path path) {
        try {
            return Files.readString(path);
        } catch (IOException exception) {
            throw new IllegalStateException(exception);
        }
    }
}
