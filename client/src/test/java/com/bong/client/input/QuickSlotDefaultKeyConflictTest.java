package com.bong.client.input;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.regex.Pattern;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** F1-F9 默认键保留区的窄型接线回归。 */
public class QuickSlotDefaultKeyConflictTest {
    private static final Path CLIENT_SOURCES = Path.of("src/main/java/com/bong/client");
    private static final Path COMBAT_KEYBINDINGS =
        CLIENT_SOURCES.resolve("combat/CombatKeybindings.java");
    private static final Path COMBAT_HUD_BOOTSTRAP =
        CLIENT_SOURCES.resolve("combat/CombatHudBootstrap.java");
    private static final Path HUD_IMMERSION =
        CLIENT_SOURCES.resolve("hud/HudImmersionControls.java");
    private static final Path NPC_INTERACTION_LOG =
        CLIENT_SOURCES.resolve("npc/NpcInteractionLogControls.java");
    private static final Path BONG_CLIENT = CLIENT_SOURCES.resolve("BongClient.java");
    private static final Pattern FUNCTION_KEY_TOKEN =
        Pattern.compile("\\b(?:GLFW\\.)?GLFW_KEY_F(\\d{1,2})\\b");
    private static final Pattern FUNCTION_KEY_OFFSET = Pattern.compile(
        "\\b(?:GLFW\\.)?GLFW_KEY_F(\\d{1,2})\\s*([+-])\\s*(\\d+)\\b"
    );
    private static final Pattern RAW_RESERVED_KEY_CODE =
        Pattern.compile("\\b(?:290|291|292|293|294|295|296|297|298)\\b");

    @Test
    void onlyExpectedQuickSlotExpressionReferencesReservedFunctionKeys() throws IOException {
        List<ReservedKeyUse> uses = new ArrayList<>();
        try (var files = Files.walk(CLIENT_SOURCES)) {
            files.filter(path -> path.toString().endsWith(".java"))
                .forEach(path -> collectReservedKeyUses(path, uses));
        }
        uses.sort(Comparator.comparing(use -> use.path().toString()));

        assertEquals(
            List.of(new ReservedKeyUse(
                Path.of("combat/CombatKeybindings.java"),
                "GLFW.GLFW_KEY_F1"
            )),
            uses,
            "全 client 只允许快捷槽起点表达式引用 F1-F9: " + uses
        );
        assertCodeContains(read(COMBAT_KEYBINDINGS), "GLFW.GLFW_KEY_F1+i");
    }

    @Test
    void productionUsesFabricInstallersAndTopLevelBootstrap() {
        assertCodeContains(read(COMBAT_KEYBINDINGS),
            "installBindings(BongKeybindRegistry.global())");
        assertCodeContains(read(HUD_IMMERSION),
            "installToggleKey(BongKeybindRegistry.global())");
        assertCodeContains(read(NPC_INTERACTION_LOG),
            "installInteractionLogKey(BongKeybindRegistry.global())");

        String client = read(BONG_CLIENT);
        assertCodeContains(client, "NpcInteractionLogControls.register()");
        assertCodeContains(client, "HudImmersionControls.register()");
        assertCodeContains(client, "CombatHudBootstrap.register()");
    }

    @Test
    void installedBindingsRemainConnectedToRealTickEntrypoints() {
        String combatKeys = read(COMBAT_KEYBINDINGS);
        assertCodeContains(combatKeys,
            "private static void onTick(MinecraftClient client){"
                + "if(client==null||client.player==null)return;consumeQuickSlotPresses()");

        String combatBootstrap = read(COMBAT_HUD_BOOTSTRAP);
        assertCodeContains(combatBootstrap,
            "CombatKeybindings.setQuickSlotHandler(CombatHudBootstrap::onQuickSlotPressed)");

        String hud = read(HUD_IMMERSION);
        assertCodeContains(hud,
            "private static void onEndClientTick(MinecraftClient client){"
                + "consumeInstalledTogglePresses(System::currentTimeMillis)");
        String npc = read(NPC_INTERACTION_LOG);
        assertCodeContains(npc,
            "private static void onEndClientTick(MinecraftClient client){if(client==null){return;}"
                + "consumeInstalledTogglePresses(client.player!=null,client.currentScreen!=null)");
    }

    private static void collectReservedKeyUses(Path path, List<ReservedKeyUse> uses) {
        String source = codeOnly(read(path));
        Path relative = CLIENT_SOURCES.relativize(path);

        var tokenMatcher = FUNCTION_KEY_TOKEN.matcher(source);
        while (tokenMatcher.find()) {
            int functionNumber = Integer.parseInt(tokenMatcher.group(1));
            if (isReservedFunctionNumber(functionNumber)) {
                uses.add(new ReservedKeyUse(relative, tokenMatcher.group()));
            }
        }

        var offsetMatcher = FUNCTION_KEY_OFFSET.matcher(source);
        while (offsetMatcher.find()) {
            int base = Integer.parseInt(offsetMatcher.group(1));
            int offset = Integer.parseInt(offsetMatcher.group(3));
            int result = offsetMatcher.group(2).equals("+") ? base + offset : base - offset;
            if (!isReservedFunctionNumber(base) && isReservedFunctionNumber(result)) {
                uses.add(new ReservedKeyUse(relative, offsetMatcher.group()));
            }
        }

        if (source.contains("new KeyBinding[") || source.contains("extends KeyBinding")) {
            var numericMatcher = RAW_RESERVED_KEY_CODE.matcher(source);
            while (numericMatcher.find()) {
                uses.add(new ReservedKeyUse(relative, numericMatcher.group()));
            }
        }
    }

    private static boolean isReservedFunctionNumber(int functionNumber) {
        return functionNumber >= 1 && functionNumber <= 9;
    }

    private static void assertCodeContains(String source, String expected) {
        assertTrue(compact(codeOnly(source)).contains(compact(expected)),
            "生产接线缺失: " + expected);
    }

    private static String codeOnly(String source) {
        return source
            .replaceAll("(?s)/\\*.*?\\*/", "")
            .replaceAll("(?m)//.*$", "");
    }

    private static String compact(String source) {
        return source.replaceAll("\\s+", "");
    }

    private static String read(Path path) {
        try {
            return Files.readString(path);
        } catch (IOException exception) {
            throw new IllegalStateException(exception);
        }
    }

    private record ReservedKeyUse(Path path, String expression) {
    }
}
