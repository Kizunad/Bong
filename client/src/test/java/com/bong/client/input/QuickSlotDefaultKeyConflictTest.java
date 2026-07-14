package com.bong.client.input;

import com.sun.source.tree.ExpressionTree;
import com.sun.source.tree.Tree;
import com.sun.source.util.TreePath;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.lwjgl.glfw.GLFW;

import javax.lang.model.element.Modifier;
import java.io.IOException;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** plan-bughunt-quick-slot-function-key-collision-v1 — F1-F9 默认键保留区回归。 */
public class QuickSlotDefaultKeyConflictTest {
    private static final Path CLIENT_SOURCES = Path.of("src/main/java").toAbsolutePath().normalize();
    private static final Path CLIENT_PACKAGE = CLIENT_SOURCES.resolve("com/bong/client");
    private static final Path COMBAT_KEYBINDINGS =
        CLIENT_PACKAGE.resolve("combat/CombatKeybindings.java");
    private static final Path COMBAT_HUD_BOOTSTRAP =
        CLIENT_PACKAGE.resolve("combat/CombatHudBootstrap.java");
    private static final Path BONG_CLIENT = CLIENT_PACKAGE.resolve("BongClient.java");
    private static final Path QUICK_SLOT_CONFIG =
        CLIENT_PACKAGE.resolve("combat/QuickSlotConfig.java");
    private static final Path HUD_IMMERSION =
        CLIENT_PACKAGE.resolve("hud/HudImmersionControls.java");
    private static final Path NPC_INTERACTION_LOG =
        CLIENT_PACKAGE.resolve("npc/NpcInteractionLogControls.java");
    private static final String KEY_BINDING_TYPE =
        "net.minecraft.client.option.KeyBinding";
    private static final String MINECRAFT_CLIENT_TYPE =
        "net.minecraft.client.MinecraftClient";
    private static final String HUD_IMMERSION_TYPE =
        "com.bong.client.hud.HudImmersionControls";
    private static final String NPC_INTERACTION_LOG_TYPE =
        "com.bong.client.npc.NpcInteractionLogControls";
    private static final String COMBAT_KEYBINDINGS_TYPE =
        "com.bong.client.combat.CombatKeybindings";
    private static final String COMBAT_HUD_BOOTSTRAP_TYPE =
        "com.bong.client.combat.CombatHudBootstrap";
    private static final String CLIENT_REQUEST_SENDER_TYPE =
        "com.bong.client.network.ClientRequestSender";
    private static final int GLFW_KEY_F1 = GLFW.GLFW_KEY_F1;
    private static final int GLFW_KEY_F9 = GLFW.GLFW_KEY_F9;

    private static JavaSourceIndex productionIndex;

    @BeforeAll
    static void indexProductionSources() throws IOException {
        productionIndex = JavaSourceIndex.load(CLIENT_SOURCES);
    }

    @Test
    void quickSlotsStillOwnFunctionKeyDefaults() {
        JavaSourceIndex.VariableDeclaration slotCount = productionIndex.unit(QUICK_SLOT_CONFIG)
            .singleDeclaration("SLOT_COUNT");
        JavaSourceIndex.KeyBindingCall quickSlot = productionIndex.singleLoopCall(
            COMBAT_KEYBINDINGS, "register");
        JavaSourceIndex.LoopEvaluation evaluation =
            productionIndex.evaluateLoopRegistration(quickSlot);

        assertEquals(Set.of(Modifier.PUBLIC, Modifier.STATIC, Modifier.FINAL),
            slotCount.tree().getModifiers().getFlags(),
            "SLOT_COUNT 必须继续是 public static final 契约常量");
        assertEquals("int", slotCount.tree().getType().toString());
        assertEquals(9, productionIndex.constantValue(slotCount.path()),
            "快捷使用栏必须继续保留 9 个槽位，对应 F1-F9");
        assertTrue(productionIndex.isRegisteredByKeyBindingHelper(quickSlot),
            "快捷槽 KeyBinding 必须继续直接交给 KeyBindingHelper.registerKeyBinding 注册");
        assertEquals(List.of(0, 1, 2, 3, 4, 5, 6, 7, 8), evaluation.loopValues(),
            "快捷槽注册循环必须从 0 到 8 各执行一次，不绑定 i++/++i 等具体写法");
        assertEquals(expectedFunctionKeys(), evaluation.defaultKeys(),
            "逐次求值后的默认键必须严格映射为 F1-F9");
        assertEquals(expectedSlotTranslations(), evaluation.translationKeys(),
            "九次注册必须逐槽映射到 quick_slot_1 至 quick_slot_9");
    }

    @Test
    void hudImmersionDefaultsUnbound() {
        JavaSourceIndex.KeyBindingCall binding = productionIndex.singleCallByTranslation(
            HUD_IMMERSION, "key.bong-client.hud_immersive_toggle");

        assertEquals(GLFW.GLFW_KEY_UNKNOWN, productionIndex.intValue(binding, 2),
            "HUD 沉浸 KeyBinding 的第三个构造参数应默认未绑定");
        assertTrue(productionIndex.isRegisteredByKeyBindingHelper(binding),
            "HUD 沉浸 KeyBinding 必须继续注册到 Controls 配置链");
        assertStringConstant(HUD_IMMERSION, "TOGGLE_KEY", "key.bong-client.hud_immersive_toggle");
    }

    @Test
    void npcInteractionLogDefaultsUnbound() {
        JavaSourceIndex.KeyBindingCall binding = productionIndex.singleCallByTranslation(
            NPC_INTERACTION_LOG,
            "key.bong-client.npc_interaction_log"
        );

        assertEquals(GLFW.GLFW_KEY_UNKNOWN, productionIndex.intValue(binding, 2),
            "NPC 交互日志 KeyBinding 的第三个构造参数应默认未绑定");
        assertTrue(productionIndex.isRegisteredByKeyBindingHelper(binding),
            "NPC 交互日志 KeyBinding 必须继续注册到 Controls 配置链");
        assertStringConstant(
            NPC_INTERACTION_LOG,
            "KEY_TRANSLATION",
            "key.bong-client.npc_interaction_log"
        );
    }

    @Test
    void hudImmersionReboundKeyRemainsWiredThroughEndTickConsumer() {
        assertEquals(1, productionIndex.endTickRegistrationCount(
            HUD_IMMERSION, HUD_IMMERSION_TYPE, "onEndClientTick"),
            "HUD 沉浸控制必须继续在 END_CLIENT_TICK 注册消费入口");

        TreePath consumer = productionIndex.singleInvocationInMethod(
            HUD_IMMERSION,
            "onEndClientTick",
            HUD_IMMERSION_TYPE,
            "consumeTogglePresses"
        );
        assertEquals(2, productionIndex.invocationArgumentCount(consumer));
        assertTrue(productionIndex.argumentReturnsFactoryMethodResult(
            consumer,
            0,
            HUD_IMMERSION_TYPE,
            "keyBinding",
            KEY_BINDING_TYPE,
            "wasPressed"
        ), "HUD 第一个实参必须直接返回 keyBinding().wasPressed() 的结果");
    }

    @Test
    void npcInteractionLogReboundKeyRemainsWiredThroughGuardedEndTickConsumer() {
        assertEquals(1, productionIndex.endTickRegistrationCount(
            NPC_INTERACTION_LOG, NPC_INTERACTION_LOG_TYPE, "onEndClientTick"),
            "NPC 交互日志必须继续在 END_CLIENT_TICK 注册消费入口");

        TreePath consumer = productionIndex.singleInvocationInMethod(
            NPC_INTERACTION_LOG,
            "onEndClientTick",
            NPC_INTERACTION_LOG_TYPE,
            "consumeTogglePresses"
        );
        assertEquals(3, productionIndex.invocationArgumentCount(consumer));
        assertTrue(productionIndex.argumentIsFieldComparedToNull(
            consumer,
            0,
            MINECRAFT_CLIENT_TYPE,
            "player",
            Tree.Kind.NOT_EQUAL_TO
        ), "NPC 第一个实参必须是 client.player != null");
        assertTrue(productionIndex.argumentIsFieldComparedToNull(
            consumer,
            1,
            MINECRAFT_CLIENT_TYPE,
            "currentScreen",
            Tree.Kind.NOT_EQUAL_TO
        ), "NPC 第二个实参必须是 client.currentScreen != null");
        assertTrue(productionIndex.argumentIsNullGuardedFieldMethodSupplier(
            consumer,
            2,
            NPC_INTERACTION_LOG_TYPE,
            "key",
            KEY_BINDING_TYPE,
            "wasPressed"
        ), "NPC 第三个实参必须直接返回 key != null && key.wasPressed() 的结果");
    }

    @Test
    void clientBootstrapAndQuickSlotOutputChainRemainReachable() {
        assertEquals(1, productionIndex.invocationCountInMethod(
            BONG_CLIENT, "onInitializeClient", NPC_INTERACTION_LOG_TYPE, "register"),
            "BongClient 必须加载 NPC 交互日志控制入口");
        assertEquals(1, productionIndex.invocationCountInMethod(
            BONG_CLIENT, "onInitializeClient", HUD_IMMERSION_TYPE, "register"),
            "BongClient 必须加载 HUD 沉浸控制入口");
        assertEquals(1, productionIndex.invocationCountInMethod(
            BONG_CLIENT, "onInitializeClient", COMBAT_HUD_BOOTSTRAP_TYPE, "register"),
            "BongClient 必须加载快捷槽 bootstrap");
        assertEquals(1, productionIndex.invocationCountInMethod(
            COMBAT_HUD_BOOTSTRAP,
            "register",
            COMBAT_KEYBINDINGS_TYPE,
            "register"
        ), "CombatHudBootstrap 必须注册快捷槽 KeyBinding");

        assertEquals(1, productionIndex.endTickRegistrationCount(
            COMBAT_KEYBINDINGS, COMBAT_KEYBINDINGS_TYPE, "onTick"),
            "快捷槽必须继续在 END_CLIENT_TICK 消费按键");
        assertTrue(productionIndex.indexedWasPressedFeedsHandler(
            COMBAT_KEYBINDINGS,
            "onTick",
            COMBAT_KEYBINDINGS_TYPE,
            "QUICK_SLOT_KEYS",
            "quickSlotHandler"
        ), "QUICK_SLOT_KEYS[i].wasPressed() 必须把同一 i 传给 quickSlotHandler.accept(i)");

        TreePath handlerSetter = productionIndex.singleInvocationInMethod(
            COMBAT_HUD_BOOTSTRAP,
            "register",
            COMBAT_KEYBINDINGS_TYPE,
            "setQuickSlotHandler"
        );
        assertTrue(productionIndex.argumentIsMethodReference(
            handlerSetter, 0, COMBAT_HUD_BOOTSTRAP_TYPE, "onQuickSlotPressed"),
            "快捷槽 handler 必须接到 CombatHudBootstrap.onQuickSlotPressed");

        TreePath send = productionIndex.singleInvocationInMethod(
            COMBAT_HUD_BOOTSTRAP,
            "onQuickSlotPressed",
            CLIENT_REQUEST_SENDER_TYPE,
            "sendUseQuickSlot"
        );
        assertTrue(productionIndex.argumentIsMethodParameter(send, 0, "slot"),
            "sendUseQuickSlot 必须发送 onQuickSlotPressed 收到的同一 slot");
    }

    @Test
    void noOtherClientBindingClaimsF1ToF9ByDefault() {
        JavaSourceIndex.KeyBindingCall quickSlot = productionIndex.singleLoopCall(
            COMBAT_KEYBINDINGS, "register");
        BindingAudit audit = auditBindings(
            productionIndex, CLIENT_SOURCES, Set.of(quickSlot));

        assertEquals(List.of(), audit.unresolved(),
            "所有生产 KeyBinding 默认键都必须可静态解析；新增动态表达式需显式建模: "
                + audit.unresolved());
        assertEquals(List.of(), audit.collisions(),
            "F1-F9 是快捷使用槽默认键保留区；其它生产 KeyBinding 不得占用: "
                + audit.collisions());
    }

    private static void assertStringConstant(Path path, String name, String expected) {
        JavaSourceIndex.VariableDeclaration declaration =
            productionIndex.unit(path).singleDeclaration(name);
        assertEquals("String", declaration.tree().getType().toString());
        assertEquals(expected, productionIndex.constantValue(declaration.path()));
    }

    private static List<Integer> expectedFunctionKeys() {
        return List.of(
            GLFW.GLFW_KEY_F1,
            GLFW.GLFW_KEY_F2,
            GLFW.GLFW_KEY_F3,
            GLFW.GLFW_KEY_F4,
            GLFW.GLFW_KEY_F5,
            GLFW.GLFW_KEY_F6,
            GLFW.GLFW_KEY_F7,
            GLFW.GLFW_KEY_F8,
            GLFW.GLFW_KEY_F9
        );
    }

    private static List<String> expectedSlotTranslations() {
        return List.of(
            "key.bong-client.quick_slot_1",
            "key.bong-client.quick_slot_2",
            "key.bong-client.quick_slot_3",
            "key.bong-client.quick_slot_4",
            "key.bong-client.quick_slot_5",
            "key.bong-client.quick_slot_6",
            "key.bong-client.quick_slot_7",
            "key.bong-client.quick_slot_8",
            "key.bong-client.quick_slot_9"
        );
    }

    private static BindingAudit auditBindings(
        JavaSourceIndex index,
        Path root,
        Set<JavaSourceIndex.KeyBindingCall> ignoredCalls
    ) {
        List<String> collisions = new ArrayList<>();
        List<String> unresolved = new ArrayList<>();
        for (JavaSourceIndex.SourceUnit unit : index.units()) {
            for (JavaSourceIndex.KeyBindingCall call : unit.calls()) {
                if (ignoredCalls.contains(call)) {
                    continue;
                }
                if (call.arguments().size() != 4) {
                    unresolved.add(location(root, call) + "=参数数量 " + call.arguments().size());
                    continue;
                }
                ExpressionTree defaultKey = call.arguments().get(2);
                Integer keyCode = index.intValue(new TreePath(call.path(), defaultKey));
                if (keyCode == null) {
                    unresolved.add(location(root, call) + "=" + defaultKey);
                } else if (keyCode >= GLFW_KEY_F1 && keyCode <= GLFW_KEY_F9) {
                    collisions.add(
                        location(root, call) + "=GLFW_KEY_F" + (keyCode - GLFW_KEY_F1 + 1));
                }
            }
        }
        return new BindingAudit(List.copyOf(collisions), List.copyOf(unresolved));
    }

    private static String location(Path root, JavaSourceIndex.KeyBindingCall call) {
        Path normalizedRoot = root.toAbsolutePath().normalize();
        return normalizedRoot.relativize(call.pathName()) + ":" + call.line();
    }

    private record BindingAudit(List<String> collisions, List<String> unresolved) {
    }
}
