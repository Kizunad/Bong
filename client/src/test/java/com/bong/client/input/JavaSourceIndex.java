package com.bong.client.input;

import com.sun.source.tree.ArrayAccessTree;
import com.sun.source.tree.AssignmentTree;
import com.sun.source.tree.BinaryTree;
import com.sun.source.tree.BlockTree;
import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.CompoundAssignmentTree;
import com.sun.source.tree.ExpressionStatementTree;
import com.sun.source.tree.ExpressionTree;
import com.sun.source.tree.ForLoopTree;
import com.sun.source.tree.IdentifierTree;
import com.sun.source.tree.LambdaExpressionTree;
import com.sun.source.tree.LiteralTree;
import com.sun.source.tree.MemberReferenceTree;
import com.sun.source.tree.MemberSelectTree;
import com.sun.source.tree.MethodInvocationTree;
import com.sun.source.tree.MethodTree;
import com.sun.source.tree.NewClassTree;
import com.sun.source.tree.ParenthesizedTree;
import com.sun.source.tree.ReturnTree;
import com.sun.source.tree.Tree;
import com.sun.source.tree.TypeCastTree;
import com.sun.source.tree.UnaryTree;
import com.sun.source.tree.VariableTree;
import com.sun.source.tree.WhileLoopTree;
import com.sun.source.util.JavacTask;
import com.sun.source.util.SourcePositions;
import com.sun.source.util.TreePath;
import com.sun.source.util.TreePathScanner;
import com.sun.source.util.Trees;

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
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * JDK 17 javac-backed source index used by client wiring contract tests.
 *
 * <p>The index resolves symbols before exposing matchers, so tests fail closed
 * when a source fixture no longer type-checks instead of silently accepting a
 * textual lookalike.</p>
 */
final class JavaSourceIndex {
    private static final String KEY_BINDING_TYPE =
        "net.minecraft.client.option.KeyBinding";
    private static final String KEY_BINDING_HELPER_TYPE =
        "net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper";
    private static final String CLIENT_TICK_EVENTS_TYPE =
        "net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents";

    record KeyBindingCall(
        Path pathName,
        long line,
        List<? extends ExpressionTree> arguments,
        TreePath path
    ) {
    }

    record VariableDeclaration(VariableTree tree, TreePath path) {
    }

    private record MethodDeclaration(MethodTree tree, TreePath path) {
    }

    record LoopEvaluation(
        List<Integer> loopValues,
        List<Integer> defaultKeys,
        List<String> translationKeys
    ) {
    }

    record SourceUnit(
        Path path,
        CompilationUnitTree tree,
        Map<String, List<VariableDeclaration>> declarations,
        Map<String, List<MethodDeclaration>> methods,
        List<KeyBindingCall> calls
    ) {
        VariableDeclaration singleDeclaration(String name) {
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

    private final Trees trees;
    private final List<SourceUnit> units;
    private final Map<Path, SourceUnit> unitsByPath;

    private JavaSourceIndex(Trees trees, List<SourceUnit> units) {
        this.trees = trees;
        this.units = List.copyOf(units);
        this.unitsByPath = new HashMap<>();
        units.forEach(unit -> unitsByPath.put(unit.path(), unit));
    }

    static JavaSourceIndex load(Path root) throws IOException {
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
            List<CompilationUnitTree> parsedTrees = new ArrayList<>();
            task.parse().forEach(tree -> {
                CompilationUnitTree unit = (CompilationUnitTree) tree;
                if (unit.getSourceFile().toUri().getScheme().equals("file")) {
                    parsedTrees.add(unit);
                }
            });
            assertNoErrors(diagnostics, "语法解析");
            task.analyze();
            assertNoErrors(diagnostics, "语义分析");
            Trees treeApi = Trees.instance(task);
            SourcePositions positions = treeApi.getSourcePositions();
            List<SourceUnit> units = parsedTrees.stream()
                .map(tree -> parseUnit(tree, positions, treeApi))
                .toList();
            return new JavaSourceIndex(treeApi, units);
        }
    }

    List<SourceUnit> units() {
        return units;
    }

    SourceUnit unit(Path path) {
        SourceUnit unit = unitsByPath.get(path.toAbsolutePath().normalize());
        assertNotNull(unit, "未在源码索引中找到 " + path);
        return unit;
    }

    KeyBindingCall singleCallByTranslation(Path path, String translationKey) {
        List<KeyBindingCall> matches = unit(path).calls().stream()
            .filter(call -> call.arguments().size() == 4)
            .filter(call -> translationKey.equals(stringValue(
                child(call.path(), call.arguments().get(0)), new HashSet<>(), Map.of())))
            .toList();
        assertEquals(1, matches.size(),
            "期望 " + path + " 恰好有一个翻译键求值为 " + translationKey + " 的 KeyBinding");
        return matches.get(0);
    }

    KeyBindingCall singleLoopCall(Path path, String methodName) {
        List<KeyBindingCall> matches = unit(path).calls().stream()
            .filter(call -> call.arguments().size() == 4)
            .filter(call -> enclosingForLoopPath(call.path()) != null)
            .filter(call -> methodName.equals(enclosingMethodName(call.path())))
            .toList();
        assertEquals(1, matches.size(),
            "期望 " + path + " 的 " + methodName + " 方法恰有一个循环注册的 KeyBinding");
        return matches.get(0);
    }

    Integer intValue(KeyBindingCall call, int argumentIndex) {
        assertTrue(argumentIndex >= 0 && argumentIndex < call.arguments().size(),
            "KeyBinding 参数索引越界: " + argumentIndex);
        return intValue(
            child(call.path(), call.arguments().get(argumentIndex)),
            new HashSet<>(),
            Map.of()
        );
    }

    Integer intValue(TreePath path) {
        return intValue(path, new HashSet<>(), Map.of());
    }

    LoopEvaluation evaluateLoopRegistration(KeyBindingCall call) {
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

    int endTickRegistrationCount(Path path, String ownerType, String handlerName) {
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
                        containsExecutable(child(current, argument), ownerType, handlerName))) {
                    count[0]++;
                }
                return super.visitMethodInvocation(invocation, unused);
            }
        }.scan(new TreePath(unit.tree()), null);
        return count[0];
    }

    TreePath singleInvocationInMethod(
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
                    trees.getElement(getCurrentPath()), targetOwnerType, targetMethodName)) {
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

    int invocationCountInMethod(
        Path path,
        String methodName,
        String targetOwnerType,
        String targetMethodName
    ) {
        MethodDeclaration method = unit(path).singleMethod(methodName);
        int[] count = {0};
        new TreePathScanner<Void, Void>() {
            @Override
            public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                if (isExecutable(
                    trees.getElement(getCurrentPath()), targetOwnerType, targetMethodName)) {
                    count[0]++;
                }
                return super.visitMethodInvocation(invocation, unused);
            }
        }.scan(method.path(), null);
        return count[0];
    }

    int invocationArgumentCount(TreePath invocationPath) {
        return invocation(invocationPath).getArguments().size();
    }

    boolean argumentReturnsFactoryMethodResult(
        TreePath invocationPath,
        int argumentIndex,
        String factoryOwnerType,
        String factoryMethodName,
        String resultOwnerType,
        String resultMethodName
    ) {
        TreePath argumentPath = argumentPath(invocationPath, argumentIndex);
        if (argumentPath == null) {
            return false;
        }
        if (argumentPath.getLeaf() instanceof MemberReferenceTree reference) {
            return isExecutable(
                trees.getElement(argumentPath), resultOwnerType, resultMethodName)
                && isExecutable(
                    trees.getElement(child(argumentPath, reference.getQualifierExpression())),
                    factoryOwnerType,
                    factoryMethodName
                );
        }
        TreePath returned = returnedExpression(argumentPath);
        return returned != null && methodResultComesFromFactory(
            returned,
            factoryOwnerType,
            factoryMethodName,
            resultOwnerType,
            resultMethodName
        );
    }

    boolean argumentIsFieldComparedToNull(
        TreePath invocationPath,
        int argumentIndex,
        String fieldOwnerType,
        String fieldName,
        Tree.Kind comparisonKind
    ) {
        TreePath argument = argumentPath(invocationPath, argumentIndex);
        return argument != null && fieldComparedToNull(
            argument, fieldOwnerType, fieldName, comparisonKind);
    }

    boolean argumentIsNullGuardedFieldMethodSupplier(
        TreePath invocationPath,
        int argumentIndex,
        String fieldOwnerType,
        String fieldName,
        String methodOwnerType,
        String methodName
    ) {
        TreePath argument = argumentPath(invocationPath, argumentIndex);
        TreePath returned = argument == null ? null : returnedExpression(argument);
        returned = unwrapParentheses(returned);
        if (returned == null || !(returned.getLeaf() instanceof BinaryTree binary)
            || binary.getKind() != Tree.Kind.CONDITIONAL_AND) {
            return false;
        }
        return fieldComparedToNull(
            child(returned, binary.getLeftOperand()),
            fieldOwnerType,
            fieldName,
            Tree.Kind.NOT_EQUAL_TO
        ) && fieldMethodInvocation(
            child(returned, binary.getRightOperand()),
            fieldOwnerType,
            fieldName,
            methodOwnerType,
            methodName
        );
    }

    boolean argumentIsMethodReference(
        TreePath invocationPath,
        int argumentIndex,
        String ownerType,
        String methodName
    ) {
        TreePath argument = argumentPath(invocationPath, argumentIndex);
        return argument != null
            && argument.getLeaf() instanceof MemberReferenceTree
            && isExecutable(trees.getElement(argument), ownerType, methodName);
    }

    boolean argumentIsMethodParameter(
        TreePath invocationPath,
        int argumentIndex,
        String parameterName
    ) {
        TreePath argument = unwrapParentheses(argumentPath(invocationPath, argumentIndex));
        Element element = argument == null ? null : trees.getElement(argument);
        return element instanceof VariableElement parameter
            && parameter.getKind() == ElementKind.PARAMETER
            && parameter.getSimpleName().contentEquals(parameterName);
    }

    boolean indexedWasPressedFeedsHandler(
        Path path,
        String methodName,
        String fieldOwnerType,
        String keyArrayFieldName,
        String handlerFieldName
    ) {
        MethodDeclaration method = unit(path).singleMethod(methodName);
        int[] matches = {0};
        new TreePathScanner<Void, Void>() {
            @Override
            public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                TreePath current = getCurrentPath();
                if (isExecutable(
                    trees.getElement(current), "java.util.function.IntConsumer", "accept")
                    && invocation.getArguments().size() == 1
                    && invocationReceiverIsField(
                        current, fieldOwnerType, handlerFieldName)) {
                    Element slot = trees.getElement(
                        child(current, invocation.getArguments().get(0)));
                    TreePath whilePath = enclosingWhileLoopPath(current);
                    if (slot instanceof VariableElement
                        && whilePath != null
                        && whileConditionReadsIndexedKey(
                            whilePath,
                            fieldOwnerType,
                            keyArrayFieldName,
                            slot
                        )) {
                        matches[0]++;
                    }
                }
                return super.visitMethodInvocation(invocation, unused);
            }
        }.scan(method.path(), null);
        return matches[0] == 1;
    }

    Object constantValue(TreePath path) {
        Element element = trees.getElement(path);
        return element instanceof VariableElement variable ? variable.getConstantValue() : null;
    }

    boolean isRegisteredByKeyBindingHelper(KeyBindingCall call) {
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

    private boolean methodResultComesFromFactory(
        TreePath expressionPath,
        String factoryOwnerType,
        String factoryMethodName,
        String resultOwnerType,
        String resultMethodName
    ) {
        TreePath expression = unwrapParentheses(expressionPath);
        if (expression == null
            || !(expression.getLeaf() instanceof MethodInvocationTree invocation)
            || !isExecutable(trees.getElement(expression), resultOwnerType, resultMethodName)
            || !(invocation.getMethodSelect() instanceof MemberSelectTree select)) {
            return false;
        }
        return isExecutable(
            trees.getElement(child(expression, select.getExpression())),
            factoryOwnerType,
            factoryMethodName
        );
    }

    private boolean fieldComparedToNull(
        TreePath expressionPath,
        String fieldOwnerType,
        String fieldName,
        Tree.Kind comparisonKind
    ) {
        TreePath expression = unwrapParentheses(expressionPath);
        if (expression == null || !(expression.getLeaf() instanceof BinaryTree binary)
            || binary.getKind() != comparisonKind) {
            return false;
        }
        TreePath left = unwrapParentheses(child(expression, binary.getLeftOperand()));
        TreePath right = unwrapParentheses(child(expression, binary.getRightOperand()));
        return (isField(left, fieldOwnerType, fieldName) && isNullLiteral(right))
            || (isNullLiteral(left) && isField(right, fieldOwnerType, fieldName));
    }

    private boolean fieldMethodInvocation(
        TreePath expressionPath,
        String fieldOwnerType,
        String fieldName,
        String methodOwnerType,
        String methodName
    ) {
        TreePath expression = unwrapParentheses(expressionPath);
        if (expression == null
            || !(expression.getLeaf() instanceof MethodInvocationTree invocation)
            || !isExecutable(trees.getElement(expression), methodOwnerType, methodName)
            || !(invocation.getMethodSelect() instanceof MemberSelectTree select)) {
            return false;
        }
        return isField(child(expression, select.getExpression()), fieldOwnerType, fieldName);
    }

    private boolean invocationReceiverIsField(
        TreePath invocationPath,
        String fieldOwnerType,
        String fieldName
    ) {
        MethodInvocationTree invocation = invocation(invocationPath);
        return invocation.getMethodSelect() instanceof MemberSelectTree select
            && isField(child(invocationPath, select.getExpression()), fieldOwnerType, fieldName);
    }

    private boolean whileConditionReadsIndexedKey(
        TreePath whilePath,
        String fieldOwnerType,
        String keyArrayFieldName,
        Element slot
    ) {
        WhileLoopTree loop = (WhileLoopTree) whilePath.getLeaf();
        TreePath condition = unwrapParentheses(child(whilePath, loop.getCondition()));
        if (condition == null
            || !(condition.getLeaf() instanceof MethodInvocationTree invocation)
            || !isExecutable(trees.getElement(condition), KEY_BINDING_TYPE, "wasPressed")
            || !(invocation.getMethodSelect() instanceof MemberSelectTree select)) {
            return false;
        }
        TreePath receiver = unwrapParentheses(child(condition, select.getExpression()));
        if (receiver == null || !(receiver.getLeaf() instanceof ArrayAccessTree access)) {
            return false;
        }
        return isField(
            child(receiver, access.getExpression()), fieldOwnerType, keyArrayFieldName)
            && slot.equals(trees.getElement(child(receiver, access.getIndex())));
    }

    private TreePath returnedExpression(TreePath supplierPath) {
        TreePath supplier = unwrapParentheses(supplierPath);
        if (supplier == null || !(supplier.getLeaf() instanceof LambdaExpressionTree lambda)) {
            return null;
        }
        if (lambda.getBody() instanceof ExpressionTree expression) {
            return child(supplier, expression);
        }
        if (!(lambda.getBody() instanceof BlockTree block)) {
            return null;
        }
        TreePath blockPath = child(supplier, block);
        List<? extends Tree> returns = block.getStatements().stream()
            .filter(statement -> statement instanceof ReturnTree)
            .toList();
        if (returns.size() != 1) {
            return null;
        }
        ReturnTree returnTree = (ReturnTree) returns.get(0);
        if (returnTree.getExpression() == null) {
            return null;
        }
        TreePath returnPath = child(blockPath, returnTree);
        return child(returnPath, returnTree.getExpression());
    }

    private TreePath argumentPath(TreePath invocationPath, int argumentIndex) {
        MethodInvocationTree invocation = invocation(invocationPath);
        if (argumentIndex < 0 || argumentIndex >= invocation.getArguments().size()) {
            return null;
        }
        return child(invocationPath, invocation.getArguments().get(argumentIndex));
    }

    private static MethodInvocationTree invocation(TreePath invocationPath) {
        assertTrue(invocationPath.getLeaf() instanceof MethodInvocationTree,
            "目标 TreePath 必须是方法调用");
        return (MethodInvocationTree) invocationPath.getLeaf();
    }

    private static TreePath unwrapParentheses(TreePath path) {
        TreePath current = path;
        while (current != null && current.getLeaf() instanceof ParenthesizedTree parenthesized) {
            current = child(current, parenthesized.getExpression());
        }
        return current;
    }

    private static TreePath enclosingWhileLoopPath(TreePath path) {
        for (TreePath current = path; current != null; current = current.getParentPath()) {
            if (current.getLeaf() instanceof WhileLoopTree) {
                return current;
            }
        }
        return null;
    }

    private boolean isField(TreePath path, String ownerType, String fieldName) {
        if (path == null) {
            return false;
        }
        Element element = trees.getElement(path);
        return element instanceof VariableElement field
            && field.getKind() == ElementKind.FIELD
            && field.getSimpleName().contentEquals(fieldName)
            && field.getEnclosingElement() instanceof TypeElement owner
            && owner.getQualifiedName().contentEquals(ownerType);
    }

    private static boolean isNullLiteral(TreePath path) {
        return path != null && path.getLeaf().getKind() == Tree.Kind.NULL_LITERAL;
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
            Class.forName(className, false, JavaSourceIndex.class.getClassLoader());
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
