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
    private static final Pattern RESERVED_FUNCTION_KEY =
        Pattern.compile("\\bGLFW\\.GLFW_KEY_F[1-9]\\b");

    @Test
    void noOtherClientSourceClaimsF1ThroughF9() throws IOException {
        List<Path> offenders = new ArrayList<>();
        try (var files = Files.walk(CLIENT_SOURCES)) {
            files.filter(path -> path.toString().endsWith(".java"))
                .filter(path -> !path.equals(COMBAT_KEYBINDINGS))
                .filter(path -> RESERVED_FUNCTION_KEY.matcher(codeOnly(read(path))).find())
                .map(CLIENT_SOURCES::relativize)
                .forEach(offenders::add);
        }
        offenders.sort(Comparator.naturalOrder());

        assertEquals(List.of(), offenders,
            "F1-F9 只能作为快捷槽默认键，其它 client 入口不得直接占用: " + offenders);
    }

    @Test
    void productionUsesFabricRegistrarsAndTopLevelBootstrap() {
        assertCodeContains(read(COMBAT_KEYBINDINGS),
            "registerQuickSlotKeys(KeyBindingHelper::registerKeyBinding)");
        assertCodeContains(read(HUD_IMMERSION),
            "registerToggleKey(KeyBindingHelper::registerKeyBinding)");
        assertCodeContains(read(NPC_INTERACTION_LOG),
            "registerInteractionLogKey(KeyBindingHelper::registerKeyBinding)");

        String client = read(BONG_CLIENT);
        assertCodeContains(client, "NpcInteractionLogControls.register()");
        assertCodeContains(client, "HudImmersionControls.register()");
        assertCodeContains(client, "CombatHudBootstrap.register()");
    }

    @Test
    void quickSlotAndReboundConsumersRemainConnected() {
        String combatKeys = read(COMBAT_KEYBINDINGS);
        assertCodeContains(combatKeys,
            "ClientTickEvents.END_CLIENT_TICK.register(CombatKeybindings::onTick)");
        assertCodeContains(combatKeys, "while(QUICK_SLOT_KEYS[i].wasPressed())");
        assertCodeContains(combatKeys, "quickSlotHandler.accept(i)");

        String combatBootstrap = read(COMBAT_HUD_BOOTSTRAP);
        assertCodeContains(combatBootstrap,
            "CombatKeybindings.setQuickSlotHandler(CombatHudBootstrap::onQuickSlotPressed)");

        String hud = read(HUD_IMMERSION);
        assertCodeContains(hud,
            "consumeTogglePresses(keyBinding()::wasPressed,System::currentTimeMillis)");
        String npc = read(NPC_INTERACTION_LOG);
        assertCodeContains(npc,
            "consumeTogglePresses(client.player!=null,client.currentScreen!=null,"
                + "()->key!=null&&key.wasPressed())");
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
}
