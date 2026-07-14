package com.bong.client.input;

import com.sun.source.tree.BinaryTree;
import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.ExpressionTree;
import com.sun.source.tree.ForLoopTree;
import com.sun.source.tree.LiteralTree;
import com.sun.source.tree.MethodInvocationTree;
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
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;
import java.io.IOException;
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
        KeyBindingCall quickSlot = productionIndex.singleCall(
            COMBAT_KEYBINDINGS,
            "\"key.bong-client.quick_slot_\"+(i+1)"
        );

        assertEquals(Set.of(Modifier.PUBLIC, Modifier.STATIC, Modifier.FINAL),
            slotCount.tree().getModifiers().getFlags(),
            "SLOT_COUNT 必须继续是 public static final 契约常量");
        assertEquals("int", slotCount.tree().getType().toString());
        assertEquals(9, productionIndex.constantValue(slotCount.path()),
            "快捷使用栏必须继续保留 9 个槽位，对应 F1-F9");
        assertEquals("GLFW.GLFW_KEY_F1+i", compact(quickSlot.arguments().get(2)),
            "快捷槽 KeyBinding 的第三个构造参数必须继续是 F1+i");
        assertTrue(productionIndex.isRegisteredByKeyBindingHelper(quickSlot),
            "快捷槽 KeyBinding 必须继续直接交给 KeyBindingHelper.registerKeyBinding 注册");

        ForLoopTree loop = enclosingForLoop(quickSlot.path());
        assertNotNull(loop, "快捷槽 KeyBinding 必须继续由注册循环创建");
        assertEquals(1, loop.getInitializer().size());
        assertEquals(1, loop.getUpdate().size());
        assertEquals("inti=0", compact(loop.getInitializer().get(0)));
        assertEquals("i<QuickSlotConfig.SLOT_COUNT", compact(loop.getCondition()));
        assertEquals("i++;", compact(loop.getUpdate().get(0)));
    }

    @Test
    void hudImmersionDefaultsUnbound() {
        KeyBindingCall binding = productionIndex.singleCall(HUD_IMMERSION, "TOGGLE_KEY");

        assertEquals("GLFW.GLFW_KEY_UNKNOWN", compact(binding.arguments().get(2)),
            "HUD 沉浸 KeyBinding 的第三个构造参数应默认未绑定");
        assertTrue(productionIndex.isRegisteredByKeyBindingHelper(binding),
            "HUD 沉浸 KeyBinding 必须继续注册到 Controls 配置链");
        assertStringConstant(HUD_IMMERSION, "TOGGLE_KEY", "key.bong-client.hud_immersive_toggle");
    }

    @Test
    void npcInteractionLogDefaultsUnbound() {
        KeyBindingCall binding = productionIndex.singleCall(
            NPC_INTERACTION_LOG,
            "KEY_TRANSLATION"
        );

        assertEquals("GLFW.GLFW_KEY_UNKNOWN", compact(binding.arguments().get(2)),
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
    void noOtherClientBindingClaimsF1ToF9ByDefault() {
        BindingAudit audit = auditBindings(productionIndex, CLIENT_SOURCES);

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
            index.singleCall(source, "\"registered\"")));
        assertFalse(index.isRegisteredByKeyBindingHelper(
            index.singleCall(source, "\"unregistered\"")),
            "移除 registerKeyBinding 包装后，注册链契约必须立即失败");
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

    private static void assertStringConstant(Path path, String name, String expected) {
        VariableDeclaration declaration = productionIndex.unit(path).singleDeclaration(name);
        assertEquals("String", declaration.tree().getType().toString());
        assertEquals(expected, productionIndex.constantValue(declaration.path()));
    }

    private static BindingAudit auditBindings(SourceIndex index, Path root) {
        List<String> collisions = new ArrayList<>();
        List<String> unresolved = new ArrayList<>();
        for (SourceUnit unit : index.units()) {
            for (KeyBindingCall call : unit.calls()) {
                if (isQuickSlotBinding(call)) {
                    continue;
                }
                if (call.arguments().size() != 4) {
                    unresolved.add(location(root, call) + "=参数数量 " + call.arguments().size());
                    continue;
                }
                ExpressionTree defaultKey = call.arguments().get(2);
                Integer keyCode = index.intValue(new TreePath(call.path(), defaultKey), new HashSet<>());
                if (keyCode == null) {
                    unresolved.add(location(root, call) + "=" + defaultKey);
                } else if (keyCode >= GLFW_KEY_F1 && keyCode <= GLFW_KEY_F9) {
                    collisions.add(location(root, call) + "=GLFW_KEY_F" + (keyCode - GLFW_KEY_F1 + 1));
                }
            }
        }
        return new BindingAudit(List.copyOf(collisions), List.copyOf(unresolved));
    }

    private static boolean isQuickSlotBinding(KeyBindingCall call) {
        return call.pathName().equals(COMBAT_KEYBINDINGS)
            && call.arguments().size() == 4
            && compact(call.arguments().get(0))
                .equals("\"key.bong-client.quick_slot_\"+(i+1)");
    }

    private static String location(Path root, KeyBindingCall call) {
        Path normalizedRoot = root.toAbsolutePath().normalize();
        return normalizedRoot.relativize(call.pathName()) + ":" + call.line();
    }

    private static ForLoopTree enclosingForLoop(TreePath path) {
        for (TreePath current = path; current != null; current = current.getParentPath()) {
            if (current.getLeaf() instanceof ForLoopTree loop) {
                return loop;
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

    private record SourceUnit(
        Path path,
        Map<String, List<VariableDeclaration>> declarations,
        List<KeyBindingCall> calls
    ) {
        private VariableDeclaration singleDeclaration(String name) {
            List<VariableDeclaration> matches = declarations.getOrDefault(name, List.of());
            assertEquals(1, matches.size(),
                "期望 " + path + " 恰好存在一个 static final " + name + " 声明");
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
                    manager.getJavaFileObjectsFromPaths(paths)
                );
                List<CompilationUnitTree> trees = new ArrayList<>();
                task.parse().forEach(tree -> trees.add((CompilationUnitTree) tree));
                assertNoErrors(diagnostics);
                task.analyze();
                Trees treeApi = Trees.instance(task);
                SourcePositions positions = treeApi.getSourcePositions();
                List<SourceUnit> units = trees.stream()
                    .map(tree -> parseUnit(tree, positions, treeApi))
                    .toList();
                return new SourceIndex(treeApi, units);
            }
        }

        private static void assertNoErrors(DiagnosticCollector<JavaFileObject> diagnostics) {
            List<String> errors = diagnostics.getDiagnostics().stream()
                .filter(diagnostic -> diagnostic.getKind() == Diagnostic.Kind.ERROR)
                .map(Diagnostic::toString)
                .toList();
            assertEquals(List.of(), errors, "生产 Java 源码 AST 解析失败: " + errors);
        }

        private List<SourceUnit> units() {
            return units;
        }

        private SourceUnit unit(Path path) {
            SourceUnit unit = unitsByPath.get(path.toAbsolutePath().normalize());
            assertNotNull(unit, "未在源码索引中找到 " + path);
            return unit;
        }

        private KeyBindingCall singleCall(Path path, String firstArgument) {
            List<KeyBindingCall> matches = unit(path).calls().stream()
                .filter(call -> call.arguments().size() == 4)
                .filter(call -> compact(call.arguments().get(0)).equals(firstArgument))
                .toList();
            assertEquals(1, matches.size(),
                "期望 " + path + " 恰好有一个首参数为 " + firstArgument + " 的 KeyBinding");
            return matches.get(0);
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

        private Integer intValue(TreePath path, Set<Element> visiting) {
            Tree tree = path.getLeaf();
            return switch (tree.getKind()) {
                case PARENTHESIZED -> intValue(child(
                    path, ((ParenthesizedTree) tree).getExpression()), visiting);
                case TYPE_CAST -> intValue(child(path, ((TypeCastTree) tree).getExpression()), visiting);
                case INT_LITERAL -> ((Number) ((LiteralTree) tree).getValue()).intValue();
                case IDENTIFIER, MEMBER_SELECT -> intConstant(path, visiting);
                case UNARY_PLUS -> intValue(child(path, ((UnaryTree) tree).getExpression()), visiting);
                case UNARY_MINUS -> negate(intValue(
                    child(path, ((UnaryTree) tree).getExpression()), visiting));
                case PLUS, MINUS -> binaryValue(path, (BinaryTree) tree, visiting);
                case METHOD_INVOCATION -> unknownKeyCode((MethodInvocationTree) tree);
                default -> null;
            };
        }

        private Integer intConstant(TreePath path, Set<Element> visiting) {
            Element element = trees.getElement(path);
            if (!(element instanceof VariableElement variable) || !visiting.add(element)) {
                return null;
            }
            Object value = variable.getConstantValue();
            visiting.remove(element);
            return value instanceof Number number ? number.intValue() : null;
        }

        private Integer binaryValue(TreePath path, BinaryTree tree, Set<Element> visiting) {
            Integer left = intValue(child(path, tree.getLeftOperand()), visiting);
            Integer right = intValue(child(path, tree.getRightOperand()), visiting);
            if (left == null || right == null) {
                return null;
            }
            return tree.getKind() == Tree.Kind.PLUS ? left + right : left - right;
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

        private static SourceUnit parseUnit(
            CompilationUnitTree tree,
            SourcePositions positions,
            Trees semanticTrees
        ) {
            Path path = Path.of(tree.getSourceFile().toUri()).toAbsolutePath().normalize();
            Map<String, List<VariableDeclaration>> declarations = new LinkedHashMap<>();
            List<KeyBindingCall> calls = new ArrayList<>();
            new TreePathScanner<Void, Void>() {
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
            return new SourceUnit(path, Map.copyOf(immutableDeclarations), List.copyOf(calls));
        }

        private static boolean isKeyBindingConstructor(Element element) {
            return element instanceof ExecutableElement constructor
                && constructor.getKind() == ElementKind.CONSTRUCTOR
                && constructor.getEnclosingElement() instanceof TypeElement owner
                && owner.getQualifiedName().contentEquals(KEY_BINDING_TYPE);
        }
    }
}
