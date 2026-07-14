package com.bong.client.input;

import com.sun.source.tree.ExpressionTree;
import com.sun.source.tree.Tree;
import com.sun.source.util.TreePath;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.lwjgl.glfw.GLFW;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** Positive and adversarial fixtures for {@link JavaSourceIndex}. */
class JavaSourceIndexTest {
    private static final String KEY_BINDING_TYPE =
        "net.minecraft.client.option.KeyBinding";
    private static final String MINECRAFT_CLIENT_TYPE =
        "net.minecraft.client.MinecraftClient";
    private static final int GLFW_KEY_F1 = GLFW.GLFW_KEY_F1;
    private static final int GLFW_KEY_F9 = GLFW.GLFW_KEY_F9;

    @Test
    void resolvesIndirectConstantsAndRejectsUnknownExpressions(@TempDir Path root)
        throws IOException {
        Path packageRoot = Files.createDirectories(root.resolve("probe"));
        Files.writeString(packageRoot.resolve("Defaults.java"), """
            package probe;
            import org.lwjgl.glfw.GLFW;
            final class Defaults {
                static final int RESERVED = GLFW.GLFW_KEY_F6;
                static final int UNUSED = GLFW.GLFW_KEY_F9;
            }
            """);
        Files.writeString(packageRoot.resolve("Probe.java"), """
            package probe;
            import net.minecraft.client.option.KeyBinding;
            import net.minecraft.client.util.InputUtil;
            import org.lwjgl.glfw.GLFW;
            import static org.lwjgl.glfw.GLFW.*;
            import static probe.Defaults.RESERVED;
            final class Probe {
                static final int LOCAL = GLFW.GLFW_KEY_F5;
                static int chooseDefault() { return GLFW.GLFW_KEY_F10; }
                void register() {
                    new KeyBinding("local", InputUtil.Type.KEYSYM, LOCAL, "probe");
                    new KeyBinding("indirect", InputUtil.Type.KEYSYM, Defaults.RESERVED, "probe");
                    new KeyBinding("static", InputUtil.Type.KEYSYM, RESERVED, "probe");
                    new KeyBinding("glfw-wildcard", InputUtil.Type.KEYSYM, GLFW_KEY_F7, "probe");
                    new net.minecraft.client.option.KeyBinding(
                        "qualified", InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_F8, "probe"
                    );
                    new KeyBinding("arithmetic", InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_F1 + 8, "probe");
                    new KeyBinding("safe", InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_F10, "probe");
                    new KeyBinding("dynamic", InputUtil.Type.KEYSYM, chooseDefault(), "probe");
                }
            }
            """);

        BindingAudit audit = auditBindings(JavaSourceIndex.load(root), root);

        assertEquals(6, audit.collisions().size(),
            "本地/跨文件常量、静态导入、全限定调用与常量运算都必须识别");
        assertEquals(1, audit.unresolved().size(),
            "无法解析的默认键表达式必须 fail closed，不能静默漏过");
        assertTrue(audit.unresolved().get(0).contains("chooseDefault()"));
    }

    @Test
    void loopEvaluatorAcceptsPrefixIncrementAndConstantAliases(@TempDir Path root)
        throws IOException {
        Path packageRoot = Files.createDirectories(root.resolve("probe"));
        Path source = packageRoot.resolve("Probe.java");
        Files.writeString(source, """
            package probe;
            import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
            import net.minecraft.client.option.KeyBinding;
            import net.minecraft.client.util.InputUtil;
            import org.lwjgl.glfw.GLFW;
            final class Probe {
                static final int SLOT_COUNT_ALIAS = 9;
                static final int FIRST_FUNCTION_KEY = GLFW.GLFW_KEY_F1;
                void register() {
                    for (int slot = 0; slot < SLOT_COUNT_ALIAS; ++slot) {
                        KeyBindingHelper.registerKeyBinding(new KeyBinding(
                            "key.bong-client.quick_slot_" + (slot + 1),
                            InputUtil.Type.KEYSYM,
                            FIRST_FUNCTION_KEY + slot,
                            "probe"
                        ));
                    }
                }
            }
            """);

        JavaSourceIndex index = JavaSourceIndex.load(root);
        JavaSourceIndex.LoopEvaluation evaluation = index.evaluateLoopRegistration(
            index.singleLoopCall(source, "register"));

        assertEquals(List.of(0, 1, 2, 3, 4, 5, 6, 7, 8), evaluation.loopValues());
        assertEquals(expectedFunctionKeys(), evaluation.defaultKeys(),
            "常量别名与 ++slot 必须按最终键值语义通过，不能退回源码字符串比较");
        assertEquals(expectedSlotTranslations(), evaluation.translationKeys());
    }

    @Test
    void registrationMatcherRejectsUnwrappedBindings(@TempDir Path root) throws IOException {
        Path packageRoot = Files.createDirectories(root.resolve("probe"));
        Path source = packageRoot.resolve("Probe.java");
        Files.writeString(source, """
            package probe;
            import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
            import net.minecraft.client.option.KeyBinding;
            import net.minecraft.client.util.InputUtil;
            final class Probe {
                void register() {
                    KeyBindingHelper.registerKeyBinding(new KeyBinding(
                        "registered", InputUtil.Type.KEYSYM, -1, "probe"
                    ));
                    new KeyBinding("unregistered", InputUtil.Type.KEYSYM, -1, "probe");
                }
            }
            """);

        JavaSourceIndex index = JavaSourceIndex.load(root);

        assertTrue(index.isRegisteredByKeyBindingHelper(
            index.singleCallByTranslation(source, "registered")));
        assertFalse(index.isRegisteredByKeyBindingHelper(
            index.singleCallByTranslation(source, "unregistered")),
            "移除 registerKeyBinding 包装后，注册链契约必须立即失败");
    }

    @Test
    void tickAdapterMatcherRejectsDiscardedReturnReversedGuardsAndSwappedArguments(
        @TempDir Path root
    ) throws IOException {
        Path packageRoot = Files.createDirectories(root.resolve("probe"));
        Path wiring = packageRoot.resolve("WiringProbe.java");
        Path missingRegistration = packageRoot.resolve("MissingRegistration.java");
        Files.writeString(wiring, """
            package probe;
            import java.util.function.BooleanSupplier;
            import java.util.function.LongSupplier;
            import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
            import net.minecraft.client.MinecraftClient;
            import net.minecraft.client.option.KeyBinding;
            final class WiringProbe {
                static KeyBinding key;
                static KeyBinding keyBinding() {
                    return key;
                }
                static void register() {
                    ClientTickEvents.END_CLIENT_TICK.register(WiringProbe::goodHud);
                }
                static void goodHud(MinecraftClient client) {
                    consumeHud(keyBinding()::wasPressed, System::currentTimeMillis);
                }
                static void discardedHud(MinecraftClient client) {
                    consumeHud(() -> {
                        keyBinding().wasPressed();
                        return false;
                    }, System::currentTimeMillis);
                }
                static void goodNpc(MinecraftClient client) {
                    consumeNpc(
                        client.player != null,
                        client.currentScreen != null,
                        () -> key != null && key.wasPressed()
                    );
                }
                static void reversedNpc(MinecraftClient client) {
                    consumeNpc(
                        client.player == null,
                        client.currentScreen == null,
                        () -> {
                            key.wasPressed();
                            return false;
                        }
                    );
                }
                static void swappedNpc(MinecraftClient client) {
                    consumeNpc(
                        client.currentScreen != null,
                        client.player != null,
                        () -> key != null && key.wasPressed()
                    );
                }
                static void consumeHud(BooleanSupplier wasPressed, LongSupplier clock) {
                }
                static void consumeNpc(
                    boolean playerPresent,
                    boolean screenOpen,
                    BooleanSupplier wasPressed
                ) {
                }
            }
            """);
        Files.writeString(missingRegistration, """
            package probe;
            import net.minecraft.client.MinecraftClient;
            final class MissingRegistration {
                static void onEndClientTick(MinecraftClient client) {
                }
            }
            """);

        JavaSourceIndex index = JavaSourceIndex.load(root);
        assertEquals(1, index.endTickRegistrationCount(
            wiring, "probe.WiringProbe", "goodHud"));
        assertEquals(0, index.endTickRegistrationCount(
            missingRegistration, "probe.MissingRegistration", "onEndClientTick"),
            "删除 END_CLIENT_TICK 注册必须被接线审计拒绝");

        TreePath goodHud = index.singleInvocationInMethod(
            wiring, "goodHud", "probe.WiringProbe", "consumeHud");
        TreePath discardedHud = index.singleInvocationInMethod(
            wiring, "discardedHud", "probe.WiringProbe", "consumeHud");
        assertTrue(index.argumentReturnsFactoryMethodResult(
            goodHud, 0, "probe.WiringProbe", "keyBinding", KEY_BINDING_TYPE, "wasPressed"));
        assertFalse(index.argumentReturnsFactoryMethodResult(
            discardedHud,
            0,
            "probe.WiringProbe",
            "keyBinding",
            KEY_BINDING_TYPE,
            "wasPressed"
        ), "调用 wasPressed 后恒定返回 false 必须被拒绝");

        TreePath goodNpc = index.singleInvocationInMethod(
            wiring, "goodNpc", "probe.WiringProbe", "consumeNpc");
        assertTrue(index.argumentIsFieldComparedToNull(
            goodNpc, 0, MINECRAFT_CLIENT_TYPE, "player", Tree.Kind.NOT_EQUAL_TO));
        assertTrue(index.argumentIsFieldComparedToNull(
            goodNpc, 1, MINECRAFT_CLIENT_TYPE, "currentScreen", Tree.Kind.NOT_EQUAL_TO));
        assertTrue(index.argumentIsNullGuardedFieldMethodSupplier(
            goodNpc, 2, "probe.WiringProbe", "key", KEY_BINDING_TYPE, "wasPressed"));

        TreePath reversedNpc = index.singleInvocationInMethod(
            wiring, "reversedNpc", "probe.WiringProbe", "consumeNpc");
        assertFalse(index.argumentIsFieldComparedToNull(
            reversedNpc, 0, MINECRAFT_CLIENT_TYPE, "player", Tree.Kind.NOT_EQUAL_TO),
            "player == null 反极性 guard 必须被拒绝");
        assertFalse(index.argumentIsFieldComparedToNull(
            reversedNpc, 1, MINECRAFT_CLIENT_TYPE, "currentScreen", Tree.Kind.NOT_EQUAL_TO),
            "currentScreen == null 反极性 guard 必须被拒绝");
        assertFalse(index.argumentIsNullGuardedFieldMethodSupplier(
            reversedNpc,
            2,
            "probe.WiringProbe",
            "key",
            KEY_BINDING_TYPE,
            "wasPressed"
        ), "调用 wasPressed 后恒定返回 false 的 NPC supplier 必须被拒绝");

        TreePath swappedNpc = index.singleInvocationInMethod(
            wiring, "swappedNpc", "probe.WiringProbe", "consumeNpc");
        assertFalse(index.argumentIsFieldComparedToNull(
            swappedNpc, 0, MINECRAFT_CLIENT_TYPE, "player", Tree.Kind.NOT_EQUAL_TO),
            "交换 player/screen 实参必须被拒绝");
        assertFalse(index.argumentIsFieldComparedToNull(
            swappedNpc, 1, MINECRAFT_CLIENT_TYPE, "currentScreen", Tree.Kind.NOT_EQUAL_TO),
            "交换 player/screen 实参必须被拒绝");
    }

    @Test
    void dispatchMatcherRejectsMissingBootstrapWrongSlotAndDisconnectedHandler(
        @TempDir Path root
    ) throws IOException {
        Path packageRoot = Files.createDirectories(root.resolve("probe"));
        Path source = packageRoot.resolve("DispatchProbe.java");
        Files.writeString(source, """
            package probe;
            import java.util.function.IntConsumer;
            import net.minecraft.client.option.KeyBinding;
            final class DispatchProbe {
                static final KeyBinding[] KEYS = new KeyBinding[9];
                static IntConsumer handler;
                static void bootstrap() {
                    Controls.register();
                }
                static void missingBootstrap() {
                }
                static void goodTick() {
                    for (int slot = 0; slot < KEYS.length; slot++) {
                        while (KEYS[slot].wasPressed()) {
                            handler.accept(slot);
                        }
                    }
                }
                static void wrongSlotTick() {
                    for (int slot = 0; slot < KEYS.length; slot++) {
                        while (KEYS[slot].wasPressed()) {
                            handler.accept(0);
                        }
                    }
                }
                static void wire() {
                    setHandler(DispatchProbe::onSlot);
                }
                static void disconnectedWire() {
                    setHandler(slot -> { });
                }
                static void sendGood(int slot) {
                    send(slot);
                }
                static void sendWrong(int slot) {
                    send(0);
                }
                static void setHandler(IntConsumer next) {
                }
                static void onSlot(int slot) {
                }
                static void send(int slot) {
                }
                static final class Controls {
                    static void register() {
                    }
                }
            }
            """);

        JavaSourceIndex index = JavaSourceIndex.load(root);
        assertEquals(1, index.invocationCountInMethod(
            source, "bootstrap", "probe.DispatchProbe.Controls", "register"));
        assertEquals(0, index.invocationCountInMethod(
            source, "missingBootstrap", "probe.DispatchProbe.Controls", "register"),
            "删除顶层 bootstrap 调用必须被拒绝");
        assertTrue(index.indexedWasPressedFeedsHandler(
            source, "goodTick", "probe.DispatchProbe", "KEYS", "handler"));
        assertFalse(index.indexedWasPressedFeedsHandler(
            source, "wrongSlotTick", "probe.DispatchProbe", "KEYS", "handler"),
            "wasPressed 槽位与 handler 参数不一致必须被拒绝");

        TreePath wire = index.singleInvocationInMethod(
            source, "wire", "probe.DispatchProbe", "setHandler");
        TreePath disconnectedWire = index.singleInvocationInMethod(
            source, "disconnectedWire", "probe.DispatchProbe", "setHandler");
        assertTrue(index.argumentIsMethodReference(
            wire, 0, "probe.DispatchProbe", "onSlot"));
        assertFalse(index.argumentIsMethodReference(
            disconnectedWire, 0, "probe.DispatchProbe", "onSlot"),
            "把 handler 改成空 lambda 必须被拒绝");

        TreePath sendGood = index.singleInvocationInMethod(
            source, "sendGood", "probe.DispatchProbe", "send");
        TreePath sendWrong = index.singleInvocationInMethod(
            source, "sendWrong", "probe.DispatchProbe", "send");
        assertTrue(index.argumentIsMethodParameter(sendGood, 0, "slot"));
        assertFalse(index.argumentIsMethodParameter(sendWrong, 0, "slot"),
            "把出料槽位改成常量必须被拒绝");
    }

    @Test
    void auditsKeyBindingSubclassSuperConstructors(@TempDir Path root)
        throws IOException {
        Path packageRoot = Files.createDirectories(root.resolve("probe"));
        Files.writeString(packageRoot.resolve("ProbeBindings.java"), """
            package probe;
            import net.minecraft.client.option.KeyBinding;
            import net.minecraft.client.util.InputUtil;
            import org.lwjgl.glfw.GLFW;
            final class ReservedBinding extends KeyBinding {
                ReservedBinding() {
                    super("reserved", InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_F6, "probe");
                }
            }
            final class SafeBinding extends KeyBinding {
                SafeBinding() {
                    super("safe", InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_F10, "probe");
                }
            }
            final class DynamicBinding extends KeyBinding {
                DynamicBinding(int key) {
                    super("dynamic", InputUtil.Type.KEYSYM, key, "probe");
                }
            }
            """);

        BindingAudit audit = auditBindings(JavaSourceIndex.load(root), root);

        assertEquals(1, audit.collisions().size(),
            "KeyBinding 子类通过 super(...) 占用 F1-F9 时必须被审计命中");
        assertTrue(audit.collisions().get(0).endsWith("=GLFW_KEY_F6"));
        assertEquals(1, audit.unresolved().size(),
            "子类 super(...) 的动态默认键同样必须 fail closed");
        assertTrue(audit.unresolved().get(0).endsWith("=key"));
    }

    @Test
    void failsClosedOnSemanticErrors(@TempDir Path root) throws IOException {
        Path packageRoot = Files.createDirectories(root.resolve("probe"));
        Files.writeString(packageRoot.resolve("BrokenProbe.java"), """
            package probe;
            import net.minecraft.client.option.MissingKeyBinding;
            final class BrokenProbe {
                void register() {
                    new MissingKeyBinding();
                }
            }
            """);

        AssertionError error = assertThrows(
            AssertionError.class, () -> JavaSourceIndex.load(root));

        assertTrue(error.getMessage().contains("语义分析失败"));
        assertTrue(error.getMessage().contains("MissingKeyBinding"));
    }

    private static BindingAudit auditBindings(JavaSourceIndex index, Path root) {
        List<String> collisions = new ArrayList<>();
        List<String> unresolved = new ArrayList<>();
        for (JavaSourceIndex.SourceUnit unit : index.units()) {
            for (JavaSourceIndex.KeyBindingCall call : unit.calls()) {
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

    private record BindingAudit(List<String> collisions, List<String> unresolved) {
    }
}
