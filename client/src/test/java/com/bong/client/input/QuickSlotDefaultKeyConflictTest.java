package com.bong.client.input;

import com.sun.source.tree.AssignmentTree;
import com.sun.source.tree.BinaryTree;
import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.CompoundAssignmentTree;
import com.sun.source.tree.ExpressionTree;
import com.sun.source.tree.ExpressionStatementTree;
import com.sun.source.tree.ForLoopTree;
import com.sun.source.tree.IdentifierTree;
import com.sun.source.tree.LiteralTree;
import com.sun.source.tree.MemberReferenceTree;
import com.sun.source.tree.MemberSelectTree;
import com.sun.source.tree.MethodInvocationTree;
import com.sun.source.tree.MethodTree;
import com.sun.source.tree.NewClassTree;
import com.sun.source.tree.ParenthesizedTree;
import com.sun.source.tree.Tree;
import com.sun.source.tree.TypeCastTree;
import com.sun.source.tree.UnaryTree;
import com.sun.source.tree.VariableTree;
import com.sun.source.util.JavacTask;
import com.sun.source.util.SourcePositions;
import com.sun.source.util.TreePath;
import com.sun.source.util.TreePathScanner;
import com.sun.source.util.Trees;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.lwjgl.glfw.GLFW;

import javax.lang.model.element.Element;
import javax.lang.model.element.ElementKind;
import javax.lang.model.element.ExecutableElement;
import javax.lang.model.element.Modifier;
import javax.lang.model.element.TypeElement;
import javax.lang.model.element.VariableElement;
import javax.tools.Diagnostic;
import javax.tools.DiagnosticCollector;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.SimpleJavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;
import java.io.IOException;
import java.net.URI;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** plan-bughunt-quick-slot-function-key-collision-v1 — F1-F9 默认键保留区回归。 */
public class QuickSlotDefaultKeyConflictTest {
    private static final Path CLIENT_SOURCES = Path.of("src/main/java").toAbsolutePath().normalize();
    private static final Path CLIENT_PACKAGE = CLIENT_SOURCES.resolve("com/bong/client");
    private static final Path COMBAT_KEYBINDINGS =
        CLIENT_PACKAGE.resolve("combat/CombatKeybindings.java");
    private static final Path QUICK_SLOT_CONFIG =
        CLIENT_PACKAGE.resolve("combat/QuickSlotConfig.java");
    private static final Path HUD_IMMERSION =
        CLIENT_PACKAGE.resolve("hud/HudImmersionControls.java");
    private static final Path NPC_INTERACTION_LOG =
        CLIENT_PACKAGE.resolve("npc/NpcInteractionLogControls.java");
    private static final String KEY_BINDING_TYPE = "net.minecraft.client.option.KeyBinding";
    private static final String KEY_BINDING_HELPER_TYPE =
        "net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper";
    private static final String CLIENT_TICK_EVENTS_TYPE =
        "net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents";
    private static final String MINECRAFT_CLIENT_TYPE =
        "net.minecraft.client.MinecraftClient";
    private static final String HUD_IMMERSION_TYPE =
        "com.bong.client.hud.HudImmersionControls";
    private static final String NPC_INTERACTION_LOG_TYPE =
        "com.bong.client.npc.NpcInteractionLogControls";
    private static final int GLFW_KEY_F1 = GLFW.GLFW_KEY_F1;
    private static final int GLFW_KEY_F9 = GLFW.GLFW_KEY_F9;

    private static SourceIndex productionIndex;

    @BeforeAll
    static void indexProductionSources() throws IOException {
        productionIndex = SourceIndex.load(CLIENT_SOURCES);
    }

    @Test
    void quickSlotsStillOwnFunctionKeyDefaults() {
        VariableDeclaration slotCount = productionIndex.unit(QUICK_SLOT_CONFIG)
            .singleDeclaration("SLOT_COUNT");
        KeyBindingCall quickSlot = productionIndex.singleLoopCall(
            COMBAT_KEYBINDINGS, "register");
        LoopEvaluation evaluation = productionIndex.evaluateLoopRegistration(quickSlot);

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
        KeyBindingCall binding = productionIndex.singleCallByTranslation(
            HUD_IMMERSION, "key.bong-client.hud_immersive_toggle");

        assertEquals(GLFW.GLFW_KEY_UNKNOWN, productionIndex.intValue(binding, 2),
            "HUD 沉浸 KeyBinding 的第三个构造参数应默认未绑定");
        assertTrue(productionIndex.isRegisteredByKeyBindingHelper(binding),
            "HUD 沉浸 KeyBinding 必须继续注册到 Controls 配置链");
        assertStringConstant(HUD_IMMERSION, "TOGGLE_KEY", "key.bong-client.hud_immersive_toggle");
    }

    @Test
    void npcInteractionLogDefaultsUnbound() {
        KeyBindingCall binding = productionIndex.singleCallByTranslation(
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
        assertTrue(productionIndex.containsExecutable(
            consumer, KEY_BINDING_TYPE, "wasPressed"),
            "tick handler 必须把真实 KeyBinding.wasPressed 接入消费函数");
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
        assertTrue(productionIndex.containsExecutable(
            consumer, KEY_BINDING_TYPE, "wasPressed"),
            "tick handler 必须把真实 KeyBinding.wasPressed 接入消费函数");
        assertEquals(Set.of("player", "currentScreen"), productionIndex.referencedFields(
            consumer,
            MINECRAFT_CLIENT_TYPE,
            Set.of("player", "currentScreen")
        ), "NPC 日志消费必须继续受玩家存在与界面关闭条件约束");
    }

    @Test
    void noOtherClientBindingClaimsF1ToF9ByDefault() {
        KeyBindingCall quickSlot = productionIndex.singleLoopCall(
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

    @Test
    void scannerResolvesIndirectConstantsAndRejectsUnknownExpressions(@TempDir Path root)
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

        BindingAudit audit = auditBindings(SourceIndex.load(root), root);

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

        SourceIndex index = SourceIndex.load(root);
        LoopEvaluation evaluation = index.evaluateLoopRegistration(
            index.singleLoopCall(source, "register"));

        assertEquals(List.of(0, 1, 2, 3, 4, 5, 6, 7, 8), evaluation.loopValues());
        assertEquals(expectedFunctionKeys(), evaluation.defaultKeys(),
            "常量别名与 ++slot 必须按最终键值语义通过，不能退回源码字符串比较");
        assertEquals(expectedSlotTranslations(), evaluation.translationKeys());
    }

    @Test
    void registrationScannerRejectsUnwrappedBindings(@TempDir Path root) throws IOException {
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

        SourceIndex index = SourceIndex.load(root);

        assertTrue(index.isRegisteredByKeyBindingHelper(
            index.singleCallByTranslation(source, "registered")));
        assertFalse(index.isRegisteredByKeyBindingHelper(
            index.singleCallByTranslation(source, "unregistered")),
            "移除 registerKeyBinding 包装后，注册链契约必须立即失败");
    }

    @Test
    void tickWiringScannerRejectsMissingRegistrationAndWasPressed(@TempDir Path root)
        throws IOException {
        Path packageRoot = Files.createDirectories(root.resolve("probe"));
        Path wired = packageRoot.resolve("Wired.java");
        Path missingRegistration = packageRoot.resolve("MissingRegistration.java");
        Path missingWasPressed = packageRoot.resolve("MissingWasPressed.java");
        Files.writeString(wired, """
            package probe;
            import java.util.function.BooleanSupplier;
            import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
            import net.minecraft.client.MinecraftClient;
            import net.minecraft.client.option.KeyBinding;
            final class Wired {
                static KeyBinding key;
                static void register() {
                    ClientTickEvents.END_CLIENT_TICK.register(Wired::onEndClientTick);
                }
                static void onEndClientTick(MinecraftClient client) {
                    consumeTogglePresses(key::wasPressed);
                }
                static void consumeTogglePresses(BooleanSupplier wasPressed) {
                }
            }
            """);
        Files.writeString(missingRegistration, """
            package probe;
            import java.util.function.BooleanSupplier;
            import net.minecraft.client.MinecraftClient;
            import net.minecraft.client.option.KeyBinding;
            final class MissingRegistration {
                static KeyBinding key;
                static void onEndClientTick(MinecraftClient client) {
                    consumeTogglePresses(key::wasPressed);
                }
                static void consumeTogglePresses(BooleanSupplier wasPressed) {
                }
            }
            """);
        Files.writeString(missingWasPressed, """
            package probe;
            import java.util.function.BooleanSupplier;
            import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
            import net.minecraft.client.MinecraftClient;
            final class MissingWasPressed {
                static void register() {
                    ClientTickEvents.END_CLIENT_TICK.register(MissingWasPressed::onEndClientTick);
                }
                static void onEndClientTick(MinecraftClient client) {
                    consumeTogglePresses(() -> false);
                }
                static void consumeTogglePresses(BooleanSupplier wasPressed) {
                }
            }
            """);

        SourceIndex index = SourceIndex.load(root);
        assertEquals(1, index.endTickRegistrationCount(
            wired, "probe.Wired", "onEndClientTick"));
        assertTrue(index.containsExecutable(index.singleInvocationInMethod(
            wired, "onEndClientTick", "probe.Wired", "consumeTogglePresses"),
            KEY_BINDING_TYPE, "wasPressed"));
        assertEquals(0, index.endTickRegistrationCount(
            missingRegistration, "probe.MissingRegistration", "onEndClientTick"),
            "删除 END_CLIENT_TICK 注册必须被接线审计拒绝");
        assertFalse(index.containsExecutable(index.singleInvocationInMethod(
            missingWasPressed,
            "onEndClientTick",
            "probe.MissingWasPressed",
            "consumeTogglePresses"
        ), KEY_BINDING_TYPE, "wasPressed"),
            "把真实 wasPressed 替换为恒假 supplier 必须被接线审计拒绝");
    }

    @Test
    void scannerAuditsKeyBindingSubclassSuperConstructors(@TempDir Path root)
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

        BindingAudit audit = auditBindings(SourceIndex.load(root), root);

        assertEquals(1, audit.collisions().size(),
            "KeyBinding 子类通过 super(...) 占用 F1-F9 时必须被审计命中");
        assertTrue(audit.collisions().get(0).endsWith("=GLFW_KEY_F6"));
        assertEquals(1, audit.unresolved().size(),
            "子类 super(...) 的动态默认键同样必须 fail closed");
        assertTrue(audit.unresolved().get(0).endsWith("=key"));
    }

    @Test
    void sourceIndexFailsClosedOnSemanticErrors(@TempDir Path root) throws IOException {
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

        AssertionError error = assertThrows(AssertionError.class, () -> SourceIndex.load(root));

        assertTrue(error.getMessage().contains("语义分析失败"));
        assertTrue(error.getMessage().contains("MissingKeyBinding"));
    }

    private static void assertStringConstant(Path path, String name, String expected) {
        VariableDeclaration declaration = productionIndex.unit(path).singleDeclaration(name);
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

    private static BindingAudit auditBindings(SourceIndex index, Path root) {
        return auditBindings(index, root, Set.of());
    }

    private static BindingAudit auditBindings(
        SourceIndex index,
        Path root,
        Set<KeyBindingCall> ignoredCalls
    ) {
        List<String> collisions = new ArrayList<>();
        List<String> unresolved = new ArrayList<>();
        for (SourceUnit unit : index.units()) {
            for (KeyBindingCall call : unit.calls()) {
                if (ignoredCalls.contains(call)) {
                    continue;
                }
                if (call.arguments().size() != 4) {
                    unresolved.add(location(root, call) + "=参数数量 " + call.arguments().size());
                    continue;
                }
                ExpressionTree defaultKey = call.arguments().get(2);
                Integer keyCode = index.intValue(
                    new TreePath(call.path(), defaultKey), new HashSet<>(), Map.of());
                if (keyCode == null) {
                    unresolved.add(location(root, call) + "=" + defaultKey);
                } else if (keyCode >= GLFW_KEY_F1 && keyCode <= GLFW_KEY_F9) {
                    collisions.add(location(root, call) + "=GLFW_KEY_F" + (keyCode - GLFW_KEY_F1 + 1));
                }
            }
        }
        return new BindingAudit(List.copyOf(collisions), List.copyOf(unresolved));
    }

    private static String location(Path root, KeyBindingCall call) {
        Path normalizedRoot = root.toAbsolutePath().normalize();
        return normalizedRoot.relativize(call.pathName()) + ":" + call.line();
    }

    private static TreePath enclosingForLoopPath(TreePath path) {
        for (TreePath current = path; current != null; current = current.getParentPath()) {
            if (current.getLeaf() instanceof ForLoopTree) {
                return current;
            }
        }
        return null;
    }

    private static String enclosingMethodName(TreePath path) {
        for (TreePath current = path; current != null; current = current.getParentPath()) {
            if (current.getLeaf() instanceof MethodTree method) {
                return method.getName().toString();
            }
        }
        return null;
    }

    private static String compact(Object value) {
        return value.toString().replaceAll("\\s+", "");
    }

    private record BindingAudit(List<String> collisions, List<String> unresolved) {
    }

    private record KeyBindingCall(
        Path pathName,
        long line,
        List<? extends ExpressionTree> arguments,
        TreePath path
    ) {
    }

    private record VariableDeclaration(VariableTree tree, TreePath path) {
    }

    private record MethodDeclaration(MethodTree tree, TreePath path) {
    }

    private record LoopEvaluation(
        List<Integer> loopValues,
        List<Integer> defaultKeys,
        List<String> translationKeys
    ) {
    }

    private record SourceUnit(
        Path path,
        CompilationUnitTree tree,
        Map<String, List<VariableDeclaration>> declarations,
        Map<String, List<MethodDeclaration>> methods,
        List<KeyBindingCall> calls
    ) {
        private VariableDeclaration singleDeclaration(String name) {
            List<VariableDeclaration> matches = declarations.getOrDefault(name, List.of());
            assertEquals(1, matches.size(),
                "期望 " + path + " 恰好存在一个 static final " + name + " 声明");
            return matches.get(0);
        }

        private MethodDeclaration singleMethod(String name) {
            List<MethodDeclaration> matches = methods.getOrDefault(name, List.of());
            assertEquals(1, matches.size(),
                "期望 " + path + " 恰好存在一个 " + name + " 方法");
            return matches.get(0);
        }
    }

    private static final class SourceIndex {
        private final Trees trees;
        private final List<SourceUnit> units;
        private final Map<Path, SourceUnit> unitsByPath;

        private SourceIndex(Trees trees, List<SourceUnit> units) {
            this.trees = trees;
            this.units = List.copyOf(units);
            this.unitsByPath = new HashMap<>();
            units.forEach(unit -> unitsByPath.put(unit.path(), unit));
        }

        private static SourceIndex load(Path root) throws IOException {
            JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
            assertNotNull(compiler, "源码契约测试必须运行在完整 JDK 17，而不是 JRE");
            List<Path> paths;
            try (var files = Files.walk(root)) {
                paths = files.filter(path -> path.toString().endsWith(".java")).sorted().toList();
            }

            DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
            try (StandardJavaFileManager manager = compiler.getStandardFileManager(
                diagnostics, Locale.ROOT, null)) {
                JavacTask task = (JavacTask) compiler.getTask(
                    null,
                    manager,
                    diagnostics,
                    List.of(
                        "-proc:none",
                        "--release", "17",
                        "-classpath", System.getProperty("java.class.path")
                    ),
                    null,
                    compilationSources(manager, paths)
                );
                List<CompilationUnitTree> trees = new ArrayList<>();
                task.parse().forEach(tree -> {
                    CompilationUnitTree unit = (CompilationUnitTree) tree;
                    if (unit.getSourceFile().toUri().getScheme().equals("file")) {
                        trees.add(unit);
                    }
                });
                assertNoErrors(diagnostics, "语法解析");
                task.analyze();
                assertNoErrors(diagnostics, "语义分析");
                Trees treeApi = Trees.instance(task);
                SourcePositions positions = treeApi.getSourcePositions();
                List<SourceUnit> units = trees.stream()
                    .map(tree -> parseUnit(tree, positions, treeApi))
                    .toList();
                return new SourceIndex(treeApi, units);
            }
        }

        private static List<JavaFileObject> compilationSources(
            StandardJavaFileManager manager,
            List<Path> paths
        ) {
            List<JavaFileObject> sources = new ArrayList<>();
            manager.getJavaFileObjectsFromPaths(paths).forEach(sources::add);
            if (!runtimeClassAvailable("org.jetbrains.annotations.Nullable")) {
                sources.add(new SourceStub(
                    "org.jetbrains.annotations.Nullable",
                    """
                        package org.jetbrains.annotations;
                        import java.lang.annotation.ElementType;
                        import java.lang.annotation.Retention;
                        import java.lang.annotation.RetentionPolicy;
                        import java.lang.annotation.Target;
                        @Target({
                            ElementType.FIELD,
                            ElementType.LOCAL_VARIABLE,
                            ElementType.METHOD,
                            ElementType.PARAMETER,
                            ElementType.TYPE_USE
                        })
                        @Retention(RetentionPolicy.CLASS)
                        public @interface Nullable {
                        }
                        """
                ));
            }
            return sources;
        }

        private static boolean runtimeClassAvailable(String className) {
            try {
                Class.forName(
                    className,
                    false,
                    QuickSlotDefaultKeyConflictTest.class.getClassLoader()
                );
                return true;
            } catch (ClassNotFoundException ignored) {
                return false;
            }
        }

        private static void assertNoErrors(
            DiagnosticCollector<JavaFileObject> diagnostics,
            String stage
        ) {
            List<String> errors = diagnostics.getDiagnostics().stream()
                .filter(diagnostic -> diagnostic.getKind() == Diagnostic.Kind.ERROR)
                .map(Diagnostic::toString)
                .toList();
            assertEquals(List.of(), errors, "生产 Java 源码" + stage + "失败: " + errors);
        }

        private List<SourceUnit> units() {
            return units;
        }

        private SourceUnit unit(Path path) {
            SourceUnit unit = unitsByPath.get(path.toAbsolutePath().normalize());
            assertNotNull(unit, "未在源码索引中找到 " + path);
            return unit;
        }

        private KeyBindingCall singleCallByTranslation(Path path, String translationKey) {
            List<KeyBindingCall> matches = unit(path).calls().stream()
                .filter(call -> call.arguments().size() == 4)
                .filter(call -> translationKey.equals(stringValue(
                    child(call.path(), call.arguments().get(0)), new HashSet<>(), Map.of())))
                .toList();
            assertEquals(1, matches.size(),
                "期望 " + path + " 恰好有一个翻译键求值为 " + translationKey + " 的 KeyBinding");
            return matches.get(0);
        }

        private KeyBindingCall singleLoopCall(Path path, String methodName) {
            List<KeyBindingCall> matches = unit(path).calls().stream()
                .filter(call -> call.arguments().size() == 4)
                .filter(call -> enclosingForLoopPath(call.path()) != null)
                .filter(call -> methodName.equals(enclosingMethodName(call.path())))
                .toList();
            assertEquals(1, matches.size(),
                "期望 " + path + " 的 " + methodName + " 方法恰有一个循环注册的 KeyBinding");
            return matches.get(0);
        }

        private Integer intValue(KeyBindingCall call, int argumentIndex) {
            assertTrue(argumentIndex >= 0 && argumentIndex < call.arguments().size(),
                "KeyBinding 参数索引越界: " + argumentIndex);
            return intValue(
                child(call.path(), call.arguments().get(argumentIndex)),
                new HashSet<>(),
                Map.of()
            );
        }

        private LoopEvaluation evaluateLoopRegistration(KeyBindingCall call) {
            TreePath loopPath = enclosingForLoopPath(call.path());
            assertNotNull(loopPath, "KeyBinding 必须位于可求值的 for 注册循环内");
            ForLoopTree loop = (ForLoopTree) loopPath.getLeaf();
            assertEquals(1, loop.getInitializer().size(),
                "快捷槽注册循环必须有唯一的整数循环变量");
            assertEquals(1, loop.getUpdate().size(),
                "快捷槽注册循环必须有唯一的步进表达式");
            assertNotNull(loop.getCondition(), "快捷槽注册循环必须有显式终止条件");

            Tree initializer = loop.getInitializer().get(0);
            assertTrue(initializer instanceof VariableTree,
                "快捷槽注册循环初始化器必须声明整数循环变量");
            VariableTree variable = (VariableTree) initializer;
            assertNotNull(variable.getInitializer(), "快捷槽循环变量必须有初值");
            TreePath variablePath = child(loopPath, variable);
            Element variableElement = trees.getElement(variablePath);
            assertTrue(variableElement instanceof VariableElement,
                "无法解析快捷槽循环变量的语义符号");
            Integer initialValue = intValue(
                child(variablePath, variable.getInitializer()), new HashSet<>(), Map.of());
            assertNotNull(initialValue, "快捷槽循环初值必须可静态求值");

            Map<Element, Integer> locals = new HashMap<>();
            locals.put(variableElement, initialValue);
            List<Integer> loopValues = new ArrayList<>();
            List<Integer> defaultKeys = new ArrayList<>();
            List<String> translationKeys = new ArrayList<>();
            TreePath conditionPath = child(loopPath, loop.getCondition());

            for (int guard = 0; guard < 100; guard++) {
                Boolean condition = booleanValue(conditionPath, locals);
                assertNotNull(condition, "快捷槽循环条件必须可按整数语义求值");
                if (!condition) {
                    return new LoopEvaluation(
                        List.copyOf(loopValues),
                        List.copyOf(defaultKeys),
                        List.copyOf(translationKeys)
                    );
                }

                loopValues.add(locals.get(variableElement));
                Integer defaultKey = intValue(
                    child(call.path(), call.arguments().get(2)), new HashSet<>(), locals);
                String translationKey = stringValue(
                    child(call.path(), call.arguments().get(0)), new HashSet<>(), locals);
                assertNotNull(defaultKey,
                    "快捷槽默认键表达式必须能在每次循环迭代中求值");
                assertNotNull(translationKey,
                    "快捷槽翻译键表达式必须能在每次循环迭代中求值");
                defaultKeys.add(defaultKey);
                translationKeys.add(translationKey);

                applyUpdate(loopPath, loop.getUpdate().get(0), locals);
            }
            throw new AssertionError("快捷槽注册循环在 100 次迭代内未终止");
        }

        private int endTickRegistrationCount(Path path, String ownerType, String handlerName) {
            SourceUnit unit = unit(path);
            int[] count = {0};
            new TreePathScanner<Void, Void>() {
                @Override
                public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                    TreePath current = getCurrentPath();
                    Element element = trees.getElement(current);
                    if (element instanceof ExecutableElement method
                        && method.getSimpleName().contentEquals("register")
                        && containsField(
                            child(current, invocation.getMethodSelect()),
                            CLIENT_TICK_EVENTS_TYPE,
                            "END_CLIENT_TICK"
                        )
                        && invocation.getArguments().stream().anyMatch(argument ->
                            containsExecutable(
                                child(current, argument), ownerType, handlerName))) {
                        count[0]++;
                    }
                    return super.visitMethodInvocation(invocation, unused);
                }
            }.scan(new TreePath(unit.tree()), null);
            return count[0];
        }

        private TreePath singleInvocationInMethod(
            Path path,
            String methodName,
            String targetOwnerType,
            String targetMethodName
        ) {
            MethodDeclaration method = unit(path).singleMethod(methodName);
            List<TreePath> matches = new ArrayList<>();
            new TreePathScanner<Void, Void>() {
                @Override
                public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                    if (isExecutable(
                        trees.getElement(getCurrentPath()),
                        targetOwnerType,
                        targetMethodName
                    )) {
                        matches.add(getCurrentPath());
                    }
                    return super.visitMethodInvocation(invocation, unused);
                }
            }.scan(method.path(), null);
            assertEquals(1, matches.size(),
                "期望 " + path + " 的 " + methodName + " 恰好调用一次 "
                    + targetOwnerType + "." + targetMethodName);
            return matches.get(0);
        }

        private boolean containsExecutable(TreePath root, String ownerType, String methodName) {
            boolean[] found = {false};
            new TreePathScanner<Void, Void>() {
                @Override
                public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                    if (isExecutable(trees.getElement(getCurrentPath()), ownerType, methodName)) {
                        found[0] = true;
                    }
                    return super.visitMethodInvocation(invocation, unused);
                }

                @Override
                public Void visitMemberReference(MemberReferenceTree reference, Void unused) {
                    if (isExecutable(trees.getElement(getCurrentPath()), ownerType, methodName)) {
                        found[0] = true;
                    }
                    return super.visitMemberReference(reference, unused);
                }
            }.scan(root, null);
            return found[0];
        }

        private Set<String> referencedFields(
            TreePath root,
            String ownerType,
            Set<String> fieldNames
        ) {
            Set<String> found = new HashSet<>();
            new TreePathScanner<Void, Void>() {
                @Override
                public Void visitIdentifier(IdentifierTree identifier, Void unused) {
                    collect(getCurrentPath());
                    return super.visitIdentifier(identifier, unused);
                }

                @Override
                public Void visitMemberSelect(MemberSelectTree memberSelect, Void unused) {
                    collect(getCurrentPath());
                    return super.visitMemberSelect(memberSelect, unused);
                }

                private void collect(TreePath path) {
                    Element element = trees.getElement(path);
                    if (element instanceof VariableElement field
                        && field.getKind() == ElementKind.FIELD
                        && fieldNames.contains(field.getSimpleName().toString())
                        && field.getEnclosingElement() instanceof TypeElement owner
                        && owner.getQualifiedName().contentEquals(ownerType)) {
                        found.add(field.getSimpleName().toString());
                    }
                }
            }.scan(root, null);
            return Set.copyOf(found);
        }

        private boolean containsField(TreePath root, String ownerType, String fieldName) {
            return referencedFields(root, ownerType, Set.of(fieldName)).contains(fieldName);
        }

        private Object constantValue(TreePath path) {
            Element element = trees.getElement(path);
            return element instanceof VariableElement variable ? variable.getConstantValue() : null;
        }

        private boolean isRegisteredByKeyBindingHelper(KeyBindingCall call) {
            TreePath parent = call.path().getParentPath();
            if (parent == null || !(parent.getLeaf() instanceof MethodInvocationTree invocation)
                || invocation.getArguments().size() != 1
                || invocation.getArguments().get(0) != call.path().getLeaf()) {
                return false;
            }
            Element element = trees.getElement(parent);
            return element instanceof ExecutableElement method
                && method.getKind() == ElementKind.METHOD
                && method.getSimpleName().contentEquals("registerKeyBinding")
                && method.getEnclosingElement() instanceof TypeElement owner
                && owner.getQualifiedName().contentEquals(KEY_BINDING_HELPER_TYPE);
        }

        private Integer intValue(
            TreePath path,
            Set<Element> visiting,
            Map<Element, Integer> locals
        ) {
            Object value = scalarValue(path, visiting, locals);
            return value instanceof Number number ? number.intValue() : null;
        }

        private String stringValue(
            TreePath path,
            Set<Element> visiting,
            Map<Element, Integer> locals
        ) {
            Object value = scalarValue(path, visiting, locals);
            return value instanceof String string ? string : null;
        }

        private Object scalarValue(
            TreePath path,
            Set<Element> visiting,
            Map<Element, Integer> locals
        ) {
            Tree tree = path.getLeaf();
            return switch (tree.getKind()) {
                case PARENTHESIZED -> scalarValue(child(
                    path, ((ParenthesizedTree) tree).getExpression()), visiting, locals);
                case TYPE_CAST -> scalarValue(
                    child(path, ((TypeCastTree) tree).getExpression()), visiting, locals);
                case INT_LITERAL, LONG_LITERAL, CHAR_LITERAL, STRING_LITERAL ->
                    ((LiteralTree) tree).getValue();
                case IDENTIFIER, MEMBER_SELECT -> variableValue(path, visiting, locals);
                case UNARY_PLUS -> intValue(
                    child(path, ((UnaryTree) tree).getExpression()), visiting, locals);
                case UNARY_MINUS -> negate(intValue(
                    child(path, ((UnaryTree) tree).getExpression()), visiting, locals));
                case PLUS, MINUS -> binaryValue(path, (BinaryTree) tree, visiting, locals);
                case METHOD_INVOCATION -> unknownKeyCode((MethodInvocationTree) tree);
                default -> null;
            };
        }

        private Object variableValue(
            TreePath path,
            Set<Element> visiting,
            Map<Element, Integer> locals
        ) {
            Element element = trees.getElement(path);
            if (!(element instanceof VariableElement variable)) {
                return null;
            }
            if (locals.containsKey(element)) {
                return locals.get(element);
            }
            if (!visiting.add(element)) {
                return null;
            }
            Object value = variable.getConstantValue();
            visiting.remove(element);
            return value;
        }

        private Object binaryValue(
            TreePath path,
            BinaryTree tree,
            Set<Element> visiting,
            Map<Element, Integer> locals
        ) {
            Object left = scalarValue(child(path, tree.getLeftOperand()), visiting, locals);
            Object right = scalarValue(child(path, tree.getRightOperand()), visiting, locals);
            if (left == null || right == null) {
                return null;
            }
            if (tree.getKind() == Tree.Kind.PLUS
                && (left instanceof String || right instanceof String)) {
                return String.valueOf(left) + right;
            }
            if (left instanceof Number leftNumber && right instanceof Number rightNumber) {
                return tree.getKind() == Tree.Kind.PLUS
                    ? leftNumber.intValue() + rightNumber.intValue()
                    : leftNumber.intValue() - rightNumber.intValue();
            }
            return null;
        }

        private Boolean booleanValue(TreePath path, Map<Element, Integer> locals) {
            if (!(path.getLeaf() instanceof BinaryTree binary)) {
                return null;
            }
            Integer left = intValue(
                child(path, binary.getLeftOperand()), new HashSet<>(), locals);
            Integer right = intValue(
                child(path, binary.getRightOperand()), new HashSet<>(), locals);
            if (left == null || right == null) {
                return null;
            }
            return switch (binary.getKind()) {
                case LESS_THAN -> left < right;
                case LESS_THAN_EQUAL -> left <= right;
                case GREATER_THAN -> left > right;
                case GREATER_THAN_EQUAL -> left >= right;
                case EQUAL_TO -> left.equals(right);
                case NOT_EQUAL_TO -> !left.equals(right);
                default -> null;
            };
        }

        private void applyUpdate(
            TreePath loopPath,
            ExpressionStatementTree update,
            Map<Element, Integer> locals
        ) {
            TreePath updatePath = child(loopPath, update);
            ExpressionTree expression = update.getExpression();
            TreePath expressionPath = child(updatePath, expression);
            switch (expression.getKind()) {
                case PREFIX_INCREMENT, POSTFIX_INCREMENT ->
                    adjustLocal(expressionPath, ((UnaryTree) expression).getExpression(), 1, locals);
                case PREFIX_DECREMENT, POSTFIX_DECREMENT ->
                    adjustLocal(expressionPath, ((UnaryTree) expression).getExpression(), -1, locals);
                case PLUS_ASSIGNMENT, MINUS_ASSIGNMENT -> {
                    CompoundAssignmentTree assignment = (CompoundAssignmentTree) expression;
                    TreePath variablePath = child(expressionPath, assignment.getVariable());
                    Element variable = trees.getElement(variablePath);
                    Integer current = locals.get(variable);
                    Integer delta = intValue(
                        child(expressionPath, assignment.getExpression()),
                        new HashSet<>(),
                        locals
                    );
                    assertNotNull(current, "快捷槽循环更新必须作用于循环变量");
                    assertNotNull(delta, "快捷槽循环复合更新量必须可静态求值");
                    locals.put(variable, expression.getKind() == Tree.Kind.PLUS_ASSIGNMENT
                        ? current + delta
                        : current - delta);
                }
                case ASSIGNMENT -> {
                    AssignmentTree assignment = (AssignmentTree) expression;
                    TreePath variablePath = child(expressionPath, assignment.getVariable());
                    Element variable = trees.getElement(variablePath);
                    assertTrue(locals.containsKey(variable),
                        "快捷槽循环赋值更新必须作用于循环变量");
                    Integer next = intValue(
                        child(expressionPath, assignment.getExpression()),
                        new HashSet<>(),
                        locals
                    );
                    assertNotNull(next, "快捷槽循环赋值更新必须可静态求值");
                    locals.put(variable, next);
                }
                default -> throw new AssertionError(
                    "不支持的快捷槽循环更新语义: " + expression.getKind());
            }
        }

        private void adjustLocal(
            TreePath expressionPath,
            ExpressionTree variableExpression,
            int delta,
            Map<Element, Integer> locals
        ) {
            Element variable = trees.getElement(child(expressionPath, variableExpression));
            Integer current = locals.get(variable);
            assertNotNull(current, "快捷槽循环自增/自减必须作用于循环变量");
            locals.put(variable, current + delta);
        }

        private static Integer unknownKeyCode(MethodInvocationTree tree) {
            String call = compact(tree);
            return call.equals("InputUtil.UNKNOWN_KEY.getCode()")
                || call.equals("net.minecraft.client.util.InputUtil.UNKNOWN_KEY.getCode()")
                ? -1
                : null;
        }

        private static Integer negate(Integer value) {
            return value == null ? null : -value;
        }

        private static TreePath child(TreePath parent, Tree child) {
            return new TreePath(parent, child);
        }

        private static boolean isExecutable(
            Element element,
            String ownerType,
            String methodName
        ) {
            return element instanceof ExecutableElement method
                && method.getSimpleName().contentEquals(methodName)
                && method.getEnclosingElement() instanceof TypeElement owner
                && owner.getQualifiedName().contentEquals(ownerType);
        }

        private static SourceUnit parseUnit(
            CompilationUnitTree tree,
            SourcePositions positions,
            Trees semanticTrees
        ) {
            Path path = Path.of(tree.getSourceFile().toUri()).toAbsolutePath().normalize();
            Map<String, List<VariableDeclaration>> declarations = new LinkedHashMap<>();
            Map<String, List<MethodDeclaration>> methods = new LinkedHashMap<>();
            List<KeyBindingCall> calls = new ArrayList<>();
            new TreePathScanner<Void, Void>() {
                @Override
                public Void visitMethod(MethodTree method, Void unused) {
                    methods.computeIfAbsent(
                        method.getName().toString(), ignored -> new ArrayList<>()).add(
                            new MethodDeclaration(method, getCurrentPath())
                        );
                    return super.visitMethod(method, unused);
                }

                @Override
                public Void visitVariable(VariableTree variable, Void unused) {
                    TreePath current = getCurrentPath();
                    if (current.getParentPath().getLeaf().getKind() == Tree.Kind.CLASS
                        && variable.getModifiers().getFlags().containsAll(
                            Set.of(Modifier.STATIC, Modifier.FINAL))) {
                        declarations.computeIfAbsent(
                            variable.getName().toString(), ignored -> new ArrayList<>()).add(
                                new VariableDeclaration(variable, current)
                            );
                    }
                    return super.visitVariable(variable, unused);
                }

                @Override
                public Void visitNewClass(NewClassTree newClass, Void unused) {
                    if (isKeyBindingConstructor(semanticTrees.getElement(getCurrentPath()))) {
                        long start = positions.getStartPosition(tree, newClass);
                        calls.add(new KeyBindingCall(
                            path,
                            tree.getLineMap().getLineNumber(start),
                            List.copyOf(newClass.getArguments()),
                            getCurrentPath()
                        ));
                    }
                    return super.visitNewClass(newClass, unused);
                }

                @Override
                public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                    if (isKeyBindingConstructor(semanticTrees.getElement(getCurrentPath()))) {
                        long start = positions.getStartPosition(tree, invocation);
                        calls.add(new KeyBindingCall(
                            path,
                            tree.getLineMap().getLineNumber(start),
                            List.copyOf(invocation.getArguments()),
                            getCurrentPath()
                        ));
                    }
                    return super.visitMethodInvocation(invocation, unused);
                }
            }.scan(tree, null);

            Map<String, List<VariableDeclaration>> immutableDeclarations = new LinkedHashMap<>();
            declarations.forEach((name, values) ->
                immutableDeclarations.put(name, List.copyOf(values)));
            Map<String, List<MethodDeclaration>> immutableMethods = new LinkedHashMap<>();
            methods.forEach((name, values) ->
                immutableMethods.put(name, List.copyOf(values)));
            return new SourceUnit(
                path,
                tree,
                Map.copyOf(immutableDeclarations),
                Map.copyOf(immutableMethods),
                List.copyOf(calls)
            );
        }

        private static boolean isKeyBindingConstructor(Element element) {
            return element instanceof ExecutableElement constructor
                && constructor.getKind() == ElementKind.CONSTRUCTOR
                && constructor.getEnclosingElement() instanceof TypeElement owner
                && owner.getQualifiedName().contentEquals(KEY_BINDING_TYPE);
        }
    }

    private static final class SourceStub extends SimpleJavaFileObject {
        private final String source;

        private SourceStub(String qualifiedName, String source) {
            super(
                URI.create("string:///" + qualifiedName.replace('.', '/') + Kind.SOURCE.extension),
                Kind.SOURCE
            );
            this.source = source;
        }

        @Override
        public CharSequence getCharContent(boolean ignoreEncodingErrors) {
            return source;
        }
    }
}
