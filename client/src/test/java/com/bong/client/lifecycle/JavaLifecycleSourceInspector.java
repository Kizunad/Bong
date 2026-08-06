package com.bong.client.lifecycle;

import com.sun.source.tree.ArrayAccessTree;
import com.sun.source.tree.ClassTree;
import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.ExpressionTree;
import com.sun.source.tree.IdentifierTree;
import com.sun.source.tree.ImportTree;
import com.sun.source.tree.MemberReferenceTree;
import com.sun.source.tree.MemberSelectTree;
import com.sun.source.tree.MethodInvocationTree;
import com.sun.source.tree.MethodTree;
import com.sun.source.tree.NewClassTree;
import com.sun.source.tree.ParenthesizedTree;
import com.sun.source.tree.PrimitiveTypeTree;
import com.sun.source.tree.Tree;
import com.sun.source.tree.TypeCastTree;
import com.sun.source.tree.VariableTree;
import com.sun.source.util.JavacTask;
import com.sun.source.util.TreePathScanner;
import com.sun.source.util.TreeScanner;

import javax.lang.model.element.Modifier;
import javax.tools.Diagnostic;
import javax.tools.DiagnosticCollector;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.SimpleJavaFileObject;
import javax.tools.ToolProvider;
import java.io.IOException;
import java.net.URI;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeSet;

public final class JavaLifecycleSourceInspector {
    private static final Set<String> TEST_RESET_METHODS = Set.of(
        "resetForTests",
        "resetForTest",
        "clearForTests"
    );
    private static final String DISCONNECT_CLEANER_METHOD = "clearOnDisconnect";

    record StoreRegistration(String storeType, String cleanerOwner, String cleanerMethod) {}

    private record MethodKey(String name, int arity) {}

    private record SourceIndex(
        Map<String, Map<String, String>> classFieldTypes,
        Map<String, Map<MethodKey, String>> classMethodReturnTypes,
        Set<String> classNames
    ) {
        private static SourceIndex from(Map<String, String> sources) {
            Map<String, Map<String, String>> fields = new HashMap<>();
            Map<String, Map<MethodKey, String>> methods = new HashMap<>();
            Set<String> classNames = new HashSet<>();

            for (Map.Entry<String, String> source : sources.entrySet()) {
                CompilationUnitTree unit = parse(simpleName(source.getKey()), source.getValue());
                String packageName = unit.getPackageName() == null ? "" : unit.getPackageName().toString();
                new TreeScanner<Void, Void>() {
                    private final List<String> classPath = new ArrayList<>();

                    @Override
                    public Void visitClass(ClassTree type, Void unused) {
                        String simpleName = type.getSimpleName().toString();
                        String nestedName = classPath.isEmpty()
                            ? simpleName
                            : String.join(".", classPath) + "." + simpleName;
                        String qualifiedName = packageName.isEmpty()
                            ? nestedName
                            : packageName + "." + nestedName;
                        classNames.add(nestedName);
                        classNames.add(qualifiedName);
                        classNames.add(simpleName);
                        classPath.add(simpleName);
                        try {
                            return super.visitClass(type, unused);
                        } finally {
                            classPath.remove(classPath.size() - 1);
                        }
                    }
                }.scan(unit, null);
            }

            for (Map.Entry<String, String> source : sources.entrySet()) {
                CompilationUnitTree unit = parse(simpleName(source.getKey()), source.getValue());
                String packageName = unit.getPackageName() == null ? "" : unit.getPackageName().toString();
                new TreeScanner<Void, Void>() {
                    private final List<String> classPath = new ArrayList<>();

                    @Override
                    public Void visitClass(ClassTree type, Void unused) {
                        String simpleName = type.getSimpleName().toString();
                        String nestedName = classPath.isEmpty()
                            ? simpleName
                            : String.join(".", classPath) + "." + simpleName;
                        String qualifiedName = packageName.isEmpty()
                            ? nestedName
                            : packageName + "." + nestedName;
                        Map<String, String> classFields = new HashMap<>();
                        Map<MethodKey, String> classMethods = new HashMap<>();
                        for (Tree member : type.getMembers()) {
                            if (member instanceof VariableTree variable && variable.getType() != null) {
                                classFields.put(
                                    variable.getName().toString(),
                                    resolveDeclaredTypeName(variable.getType().toString(), unit, classNames)
                                );
                            }
                            if (member instanceof MethodTree method && method.getReturnType() != null) {
                                classMethods.put(
                                    new MethodKey(method.getName().toString(), method.getParameters().size()),
                                    resolveDeclaredTypeName(method.getReturnType().toString(), unit, classNames)
                                );
                            }
                        }
                        putIfAbsent(fields, nestedName, classFields);
                        putIfAbsent(fields, qualifiedName, classFields);
                        putIfAbsent(fields, simpleName, classFields);
                        putIfAbsent(methods, nestedName, classMethods);
                        putIfAbsent(methods, qualifiedName, classMethods);
                        putIfAbsent(methods, simpleName, classMethods);
                        classPath.add(simpleName);
                        try {
                            return super.visitClass(type, unused);
                        } finally {
                            classPath.remove(classPath.size() - 1);
                        }
                    }
                }.scan(unit, null);
            }
            return new SourceIndex(fields, methods, classNames);
        }

        private static String resolveDeclaredTypeName(
            String declaredType,
            CompilationUnitTree unit,
            Set<String> classNames
        ) {
            String type = rawTypeName(declaredType);
            if (type.contains(".")) {
                return type;
            }
            for (ImportTree importTree : unit.getImports()) {
                if (!importTree.isStatic()
                    && importTree.getQualifiedIdentifier().toString().endsWith("." + type)) {
                    return importTree.getQualifiedIdentifier().toString();
                }
            }
            String packageName = unit.getPackageName() == null ? "" : unit.getPackageName().toString();
            String samePackageType = packageName.isEmpty() ? type : packageName + "." + type;
            return classNames.contains(samePackageType) ? samePackageType : type;
        }

        private static <K, V> void putIfAbsent(Map<String, Map<K, V>> target, String key, Map<K, V> value) {
            target.putIfAbsent(key, value);
        }
    }

    private record ResolvedType(String type, boolean known) {}

    private JavaLifecycleSourceInspector() {}

    static List<StoreRegistration> storeRegistrations(String source) {
        CompilationUnitTree unit = parse("SessionScopedStoreRegistry", source);
        List<StoreRegistration> registrations = new ArrayList<>();
        new TreeScanner<Void, Void>() {
            @Override
            public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                if (!invocation.getMethodSelect().toString().endsWith("SessionStoreHandle.forStore")) {
                    return super.visitMethodInvocation(invocation, unused);
                }
                if (invocation.getArguments().size() != 2
                    || !(invocation.getArguments().get(0) instanceof MemberSelectTree classToken)
                    || !classToken.getIdentifier().contentEquals("class")
                    || !(invocation.getArguments().get(1) instanceof MemberReferenceTree cleaner)) {
                    throw new AssertionError("无法解析 Store lifecycle 登记：" + invocation);
                }
                registrations.add(new StoreRegistration(
                    classToken.getExpression().toString(),
                    cleaner.getQualifierExpression().toString(),
                    cleaner.getName().toString()
                ));
                return super.visitMethodInvocation(invocation, unused);
            }
        }.scan(unit, null);
        return List.copyOf(registrations);
    }

    public static List<String> disconnectCleanupRegistrations(String source) {
        CompilationUnitTree unit = parse("BongNetworkHandler", source);
        List<String> registrations = new ArrayList<>();
        new TreeScanner<Void, Void>() {
            private boolean inAdjunctTeardown;

            @Override
            public Void visitMethod(MethodTree method, Void unused) {
                boolean previous = inAdjunctTeardown;
                inAdjunctTeardown = method.getName().contentEquals("runAdjunctDisconnectTeardown")
                    && method.getParameters().isEmpty();
                try {
                    return super.visitMethod(method, unused);
                } finally {
                    inAdjunctTeardown = previous;
                }
            }

            @Override
            public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                if (inAdjunctTeardown
                    && invocation.getMethodSelect().toString().equals("runDisconnectCleanups")) {
                    if (!registrations.isEmpty()) {
                        throw new AssertionError("中央 adjunct owner 只能调用一次 runDisconnectCleanups");
                    }
                    invocation.getArguments().forEach(argument -> registrations.add(
                        argument.toString().replaceAll("\\s+", "")
                    ));
                }
                return super.visitMethodInvocation(invocation, unused);
            }
        }.scan(unit, null);
        if (registrations.isEmpty()) {
            throw new AssertionError("无法定位中央 adjunct disconnect 注册表");
        }
        return List.copyOf(registrations);
    }

    static void assertDeclaresProductionCleaner(
        String source,
        String entryMethod,
        String storeIdentity
    ) {
        CompilationUnitTree unit = parse(simpleName(storeIdentity), source);
        String className = simpleName(storeIdentity);
        ClassTree storeType = unit.getTypeDecls().stream()
            .filter(ClassTree.class::isInstance)
            .map(ClassTree.class::cast)
            .filter(type -> type.getSimpleName().contentEquals(className))
            .findFirst()
            .orElseThrow(() -> new AssertionError("无法定位 production Store 类型：" + storeIdentity));
        boolean declaresEntry = storeType.getMembers().stream()
            .filter(MethodTree.class::isInstance)
            .map(MethodTree.class::cast)
            .anyMatch(method -> method.getName().contentEquals(entryMethod)
                && method.getParameters().isEmpty()
                && method.getModifiers().getFlags().contains(Modifier.PUBLIC)
                && method.getModifiers().getFlags().contains(Modifier.STATIC)
                && method.getReturnType() instanceof PrimitiveTypeTree returnType
                && returnType.getPrimitiveTypeKind() == javax.lang.model.type.TypeKind.VOID);
        if (!declaresEntry) {
            throw new AssertionError("必须能定位 production cleaner：" + storeIdentity + "." + entryMethod);
        }
    }

    static void assertNoTestResetCalls(String source, String sourceIdentity) {
        assertNoTestResetCalls(parse(simpleName(sourceIdentity), source), sourceIdentity);
    }

    private static void assertNoTestResetCalls(CompilationUnitTree unit, String sourceIdentity) {
        String sourceFqcn = sourceIdentity
            .replace('\\', '.')
            .replace('/', '.')
            .replaceFirst("\\.java$", "");
        Set<String> externalStaticResetMethods = new HashSet<>();
        boolean wildcardStaticResetImport = false;
        for (ImportTree importTree : unit.getImports()) {
            String imported = importTree.getQualifiedIdentifier().toString();
            if (!importTree.isStatic()) {
                continue;
            }
            if (imported.endsWith(".*")) {
                wildcardStaticResetImport = true;
            } else if (TEST_RESET_METHODS.contains(imported.substring(imported.lastIndexOf('.') + 1))) {
                externalStaticResetMethods.add(imported.substring(imported.lastIndexOf('.') + 1));
            }
        }
        final boolean hasWildcardStaticResetImport = wildcardStaticResetImport;
        new TreeScanner<Void, Void>() {
            private String enclosingClass;
            private String enclosingMethod;

            @Override
            public Void visitClass(ClassTree type, Void unused) {
                String previousClass = enclosingClass;
                enclosingClass = type.getSimpleName().toString();
                try {
                    return super.visitClass(type, unused);
                } finally {
                    enclosingClass = previousClass;
                }
            }

            @Override
            public Void visitMethod(MethodTree method, Void unused) {
                String previousMethod = enclosingMethod;
                enclosingMethod = method.getName().toString();
                try {
                    return super.visitMethod(method, unused);
                } finally {
                    enclosingMethod = previousMethod;
                }
            }

            @Override
            public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                rejectTestResetCall(
                    invokedMethodName(invocation.getMethodSelect()),
                    sourceIdentity,
                    enclosingClass,
                    enclosingMethod,
                    invocation.getMethodSelect().toString(),
                    sourceFqcn,
                    externalStaticResetMethods,
                    hasWildcardStaticResetImport
                );
                return super.visitMethodInvocation(invocation, unused);
            }

            @Override
            public Void visitMemberReference(MemberReferenceTree reference, Void unused) {
                rejectTestResetCall(
                    reference.getName().toString(),
                    sourceIdentity,
                    enclosingClass,
                    enclosingMethod,
                    reference.getQualifierExpression().toString() + "::" + reference.getName(),
                    sourceFqcn,
                    externalStaticResetMethods,
                    hasWildcardStaticResetImport
                );
                return super.visitMemberReference(reference, unused);
            }
        }.scan(unit, null);
    }

    static void assertProductionLifecycleContracts(
        String source,
        String sourceIdentity,
        Set<String> storeFqcns,
        boolean registrySource
    ) {
        assertProductionLifecycleContracts(
            Map.of(sourceIdentity, source),
            storeFqcns,
            registrySource ? sourceIdentity : null
        );
    }

    static void assertProductionLifecycleContracts(
        Map<String, String> sources,
        Set<String> storeFqcns,
        String registrySourceIdentity
    ) {
        SourceIndex sourceIndex = SourceIndex.from(sources);
        for (Map.Entry<String, String> entry : sources.entrySet()) {
            String sourceIdentity = entry.getKey();
            CompilationUnitTree unit = parse(simpleName(sourceIdentity), entry.getValue());
            assertNoTestResetCalls(unit, sourceIdentity);
            assertRegistryOwnsManagedStoreCleanerCalls(
                unit,
                sourceIdentity,
                storeFqcns,
                sourceIdentity.equals(registrySourceIdentity),
                sourceIndex
            );
        }
    }

    static void assertRegistryOwnsManagedStoreCleanerCalls(
        String source,
        String sourceIdentity,
        Set<String> storeFqcns,
        boolean registrySource
    ) {
        assertRegistryOwnsManagedStoreCleanerCalls(
            parse(simpleName(sourceIdentity), source),
            sourceIdentity,
            storeFqcns,
            registrySource,
            SourceIndex.from(Map.of(sourceIdentity, source))
        );
    }

    private static void assertRegistryOwnsManagedStoreCleanerCalls(
        CompilationUnitTree unit,
        String sourceIdentity,
        Set<String> storeFqcns,
        boolean registrySource,
        SourceIndex sourceIndex
    ) {
        String sourcePackage = unit.getPackageName() == null ? "" : unit.getPackageName().toString();
        String sourceFqcn = sourceIdentity
            .replace('\\', '.')
            .replace('/', '.')
            .replaceFirst("\\.java$", "");
        boolean sourceIsManagedStore = storeFqcns.contains(sourceFqcn);
        Set<String> visibleStoreTypeNames = new TreeSet<>();
        Set<String> staticallyImportedCleanupMethods = new TreeSet<>();
        Set<String> forbiddenWildcardImports = new TreeSet<>();
        for (String fqcn : storeFqcns) {
            if (packageName(fqcn).equals(sourcePackage)) {
                visibleStoreTypeNames.add(simpleName(fqcn));
            }
        }
        for (ImportTree importTree : unit.getImports()) {
            String imported = importTree.getQualifiedIdentifier().toString();
            for (String fqcn : storeFqcns) {
                if (!importTree.isStatic() && imported.equals(fqcn)) {
                    visibleStoreTypeNames.add(simpleName(fqcn));
                }
                if (!importTree.isStatic() && imported.equals(packageName(fqcn) + ".*")) {
                    forbiddenWildcardImports.add(imported);
                }
                if (importTree.isStatic() && imported.equals(fqcn + ".*")) {
                    forbiddenWildcardImports.add("static " + imported);
                }
                if (importTree.isStatic() && imported.equals(fqcn + "." + DISCONNECT_CLEANER_METHOD)) {
                    staticallyImportedCleanupMethods.add(DISCONNECT_CLEANER_METHOD);
                }
            }
        }

        Set<String> violations = new TreeSet<>();
        forbiddenWildcardImports.forEach(value -> violations.add("wildcard-import:" + value));
        new TreePathScanner<Void, Void>() {
            private String enclosingClass;
            private String enclosingMethod;
            private final ArrayDeque<Map<String, ResolvedType>> variableTypeScopes = new ArrayDeque<>();

            @Override
            public Void visitClass(ClassTree type, Void unused) {
                String previousClass = enclosingClass;
                enclosingClass = type.getSimpleName().toString();
                Map<String, ResolvedType> fieldTypes = new HashMap<>();
                for (Tree member : type.getMembers()) {
                    if (member instanceof VariableTree variable && variable.getType() != null) {
                        fieldTypes.put(
                            variable.getName().toString(),
                            resolvedDeclaredType(variable.getType().toString(), sourceIndex)
                        );
                    }
                }
                variableTypeScopes.push(fieldTypes);
                try {
                    return super.visitClass(type, unused);
                } finally {
                    variableTypeScopes.pop();
                    enclosingClass = previousClass;
                }
            }

            @Override
            public Void visitMethod(MethodTree method, Void unused) {
                String previousMethod = enclosingMethod;
                enclosingMethod = method.getName().toString();
                Map<String, ResolvedType> parameterTypes = new HashMap<>();
                for (VariableTree parameter : method.getParameters()) {
                    if (parameter.getType() != null) {
                        parameterTypes.put(
                            parameter.getName().toString(),
                            resolvedDeclaredType(parameter.getType().toString(), sourceIndex)
                        );
                    }
                }
                variableTypeScopes.push(parameterTypes);
                try {
                    return super.visitMethod(method, unused);
                } finally {
                    variableTypeScopes.pop();
                    enclosingMethod = previousMethod;
                }
            }

            @Override
            public Void visitBlock(com.sun.source.tree.BlockTree block, Void unused) {
                variableTypeScopes.push(new HashMap<>());
                try {
                    return super.visitBlock(block, unused);
                } finally {
                    variableTypeScopes.pop();
                }
            }

            @Override
            public Void visitVariable(VariableTree variable, Void unused) {
                if (!variableTypeScopes.isEmpty()) {
                    ResolvedType declaredType = variable.getType() == null
                        ? new ResolvedType(null, false)
                        : resolvedDeclaredType(variable.getType().toString(), sourceIndex);
                    if ((variable.getType() == null || variable.getType().toString().equals("var"))
                        && variable.getInitializer() != null) {
                        declaredType = inferExpressionType(variable.getInitializer());
                    }
                    variableTypeScopes.peek().put(variable.getName().toString(), declaredType);
                }
                return super.visitVariable(variable, unused);
            }

            @Override
            public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                String methodName = invokedMethodName(invocation.getMethodSelect());
                if (!methodName.equals(DISCONNECT_CLEANER_METHOD)) {
                    return super.visitMethodInvocation(invocation, unused);
                }
                if (invocation.getMethodSelect() instanceof MemberSelectTree selection) {
                    ResolvedType resolvedOwner = resolveScopedReceiverType(selection.getExpression());
                    if (!resolvedOwner.known()) {
                        violations.add("unresolved-receiver:" + invocation.getMethodSelect());
                    } else if (isForbiddenManagedStoreCall(
                        resolvedOwner.type(),
                        storeFqcns,
                        visibleStoreTypeNames,
                        sourceIsManagedStore
                    )) {
                        violations.add("invoke:" + invocation.getMethodSelect());
                    }
                } else if (invocation.getMethodSelect() instanceof IdentifierTree) {
                    if (staticallyImportedCleanupMethods.contains(methodName)) {
                        violations.add("static-import:" + methodName);
                    } else if (sourceIsManagedStore) {
                        violations.add("self-invoke:" + methodName);
                    }
                }
                return super.visitMethodInvocation(invocation, unused);
            }

            @Override
            public Void visitMemberReference(MemberReferenceTree reference, Void unused) {
                String qualifier = reference.getQualifierExpression().toString();
                if (!reference.getName().contentEquals(DISCONNECT_CLEANER_METHOD)) {
                    return super.visitMemberReference(reference, unused);
                }
                ResolvedType resolvedOwner = resolveScopedReceiverType(reference.getQualifierExpression());
                if (!resolvedOwner.known()) {
                    violations.add("unresolved-reference:" + qualifier + "::" + reference.getName());
                } else if (isManagedStoreOwner(resolvedOwner.type(), storeFqcns, visibleStoreTypeNames)) {
                    Tree parent = getCurrentPath().getParentPath().getLeaf();
                    if (!registrySource || !isSanctionedRegistryBinding(reference, parent)) {
                        violations.add("reference:" + qualifier + "::" + reference.getName());
                    }
                }
                return super.visitMemberReference(reference, unused);
            }

            private ResolvedType inferExpressionType(ExpressionTree expression) {
                if (expression instanceof ParenthesizedTree parenthesized) {
                    return inferExpressionType(parenthesized.getExpression());
                }
                if (expression instanceof TypeCastTree cast) {
                    return resolvedDeclaredType(cast.getType().toString(), sourceIndex);
                }
                if (expression instanceof NewClassTree newClass) {
                    return resolvedDeclaredType(newClass.getIdentifier().toString(), sourceIndex);
                }
                if (expression instanceof IdentifierTree identifier) {
                    return resolveScopedReceiverType(identifier.getName().toString());
                }
                if (expression instanceof MemberSelectTree selection) {
                    return resolveScopedReceiverType(selection);
                }
                if (expression instanceof MethodInvocationTree invocation) {
                    return resolveMethodReturnType(invocation);
                }
                if (expression instanceof ArrayAccessTree arrayAccess) {
                    ResolvedType arrayType = resolveScopedReceiverType(arrayAccess.getExpression());
                    if (!arrayType.known()) {
                        return arrayType;
                    }
                    return new ResolvedType(arrayType.type(), true);
                }
                return new ResolvedType(null, false);
            }

            private ResolvedType resolveScopedReceiverType(ExpressionTree expression) {
                if (expression instanceof ParenthesizedTree parenthesized) {
                    return resolveScopedReceiverType(parenthesized.getExpression());
                }
                if (expression instanceof TypeCastTree cast) {
                    return resolvedDeclaredType(cast.getType().toString(), sourceIndex);
                }
                if (expression instanceof IdentifierTree identifier) {
                    ResolvedType resolved = resolveScopedReceiverType(identifier.getName().toString());
                    if (resolved.known()) {
                        return resolved;
                    }
                    String typeName = identifier.getName().toString();
                    return sourceIndex.classNames().contains(typeName) || visibleStoreTypeNames.contains(typeName)
                        ? new ResolvedType(typeName, true)
                        : resolved;
                }
                if (expression instanceof MemberSelectTree selection) {
                    String expressionText = selection.toString();
                    if (sourceIndex.classNames().contains(expressionText)) {
                        return new ResolvedType(expressionText, true);
                    }
                    String resolved = resolveQualifiedFieldType(selection);
                    return resolved == null
                        ? new ResolvedType(null, false)
                        : new ResolvedType(resolved, true);
                }
                if (expression instanceof MethodInvocationTree invocation) {
                    return resolveMethodReturnType(invocation);
                }
                if (expression instanceof ArrayAccessTree arrayAccess) {
                    ResolvedType arrayType = resolveScopedReceiverType(arrayAccess.getExpression());
                    return arrayType.known()
                        ? new ResolvedType(arrayType.type(), true)
                        : arrayType;
                }
                return new ResolvedType(null, false);
            }

            private ResolvedType resolveScopedReceiverType(String owner) {
                String normalizedOwner = owner.trim();
                while (normalizedOwner.startsWith("(") && normalizedOwner.endsWith(")")) {
                    normalizedOwner = normalizedOwner.substring(1, normalizedOwner.length() - 1).trim();
                }
                int castEnd = castTypeEnd(normalizedOwner);
                if (castEnd >= 0) {
                    return resolvedDeclaredType(normalizedOwner.substring(1, castEnd), sourceIndex);
                }
                String variableName = normalizedOwner.startsWith("this.") || normalizedOwner.startsWith("super.")
                    ? normalizedOwner.substring(normalizedOwner.indexOf('.') + 1)
                    : normalizedOwner;
                if (!variableName.matches("[A-Za-z_$][A-Za-z0-9_$]*")) {
                    return new ResolvedType(null, false);
                }
                for (Map<String, ResolvedType> scope : variableTypeScopes) {
                    ResolvedType declaredType = scope.get(variableName);
                    if (declaredType != null) {
                        return declaredType;
                    }
                }
                return new ResolvedType(null, false);
            }

            private String resolveQualifiedFieldType(MemberSelectTree selection) {
                String member = selection.getIdentifier().toString();
                ExpressionTree receiver = selection.getExpression();
                if (receiver instanceof IdentifierTree identifier
                    && (identifier.getName().contentEquals("this") || identifier.getName().contentEquals("super"))) {
                    ResolvedType currentFieldType = resolveCurrentClassFieldType(member);
                    return currentFieldType.known() ? currentFieldType.type() : null;
                }
                ResolvedType receiverType = resolveScopedReceiverType(receiver);
                if (!receiverType.known()) {
                    return null;
                }
                Map<String, String> fields = sourceIndex.classFieldTypes().get(receiverType.type());
                if (fields == null) {
                    fields = sourceIndex.classFieldTypes().get(simpleTypeName(receiverType.type()));
                }
                return fields == null ? null : fields.get(member);
            }

            private ResolvedType resolveCurrentClassFieldType(String fieldName) {
                if (enclosingClass == null) {
                    return new ResolvedType(null, false);
                }
                Map<String, String> fields = sourceIndex.classFieldTypes().get(enclosingClass);
                String fieldType = fields == null ? null : fields.get(fieldName);
                return fieldType == null ? new ResolvedType(null, false) : new ResolvedType(fieldType, true);
            }

            private ResolvedType resolveMethodReturnType(MethodInvocationTree invocation) {
                String methodName = invokedMethodName(invocation.getMethodSelect());
                String ownerType;
                if (invocation.getMethodSelect() instanceof MemberSelectTree selection) {
                    ExpressionTree receiver = selection.getExpression();
                    ownerType = receiver instanceof IdentifierTree identifier
                        && (identifier.getName().contentEquals("this") || identifier.getName().contentEquals("super"))
                        ? enclosingClass
                        : resolveScopedReceiverType(receiver).type();
                } else {
                    ownerType = enclosingClass;
                }
                if (ownerType == null) {
                    return new ResolvedType(null, false);
                }
                Map<MethodKey, String> methods = sourceIndex.classMethodReturnTypes().get(ownerType);
                if (methods == null) {
                    methods = sourceIndex.classMethodReturnTypes().get(simpleTypeName(ownerType));
                }
                if (methods == null) {
                    return new ResolvedType(null, false);
                }
                String returnType = methods.get(new MethodKey(methodName, invocation.getArguments().size()));
                return returnType == null
                    ? new ResolvedType(null, false)
                    : new ResolvedType(returnType, true);
            }

            private ResolvedType resolvedDeclaredType(String declaredType, SourceIndex ignored) {
                String type = rawTypeName(declaredType);
                return sourceIndex.classNames().contains(type) || sourceIndex.classNames().contains(simpleTypeName(type))
                    ? new ResolvedType(type, true)
                    : new ResolvedType(type, true);
            }

            private String simpleTypeName(String typeName) {
                int separator = typeName.lastIndexOf('.');
                return separator >= 0 ? typeName.substring(separator + 1) : typeName;
            }

            private int castTypeEnd(String expression) {
                if (!expression.startsWith("(")) {
                    return -1;
                }
                int close = expression.indexOf(')');
                if (close <= 1 || close == expression.length() - 1) {
                    return -1;
                }
                String typeText = expression.substring(1, close).trim();
                return typeText.matches("[A-Za-z_$][A-Za-z0-9_$.]*(?:\\[\\])?") ? close : -1;
            }
        }.scan(unit, null);

        if (!violations.isEmpty()) {
            throw new AssertionError(
                sourceIdentity + " 不得绕过 SessionScopedStoreRegistry 清理 registry-managed Store：" + violations
            );
        }
    }

    private static boolean isSanctionedRegistryBinding(MemberReferenceTree reference, Tree parent) {
        if (!reference.getName().contentEquals("clearOnDisconnect")
            || !(parent instanceof MethodInvocationTree invocation)
            || !invocation.getMethodSelect().toString().equals("SessionStoreHandle.forStore")
            || invocation.getArguments().size() != 2
            || invocation.getArguments().get(1) != reference
            || !(invocation.getArguments().get(0) instanceof MemberSelectTree classToken)
            || !classToken.getIdentifier().contentEquals("class")) {
            return false;
        }
        return classToken.getExpression().toString().equals(reference.getQualifierExpression().toString());
    }

    private static boolean isForbiddenManagedStoreCall(
        String owner,
        Set<String> storeFqcns,
        Set<String> visibleStoreTypeNames,
        boolean sourceIsManagedStore
    ) {
        return isManagedStoreOwner(owner, storeFqcns, visibleStoreTypeNames)
            || (sourceIsManagedStore && (owner.equals("this") || owner.equals("super")));
    }

    private static boolean isManagedStoreOwner(
        String owner,
        Set<String> storeFqcns,
        Set<String> visibleStoreTypeNames
    ) {
        return storeFqcns.contains(owner) || visibleStoreTypeNames.contains(owner);
    }

    private static String invokedMethodName(Tree methodSelect) {
        if (methodSelect instanceof IdentifierTree identifier) {
            return identifier.getName().toString();
        }
        if (methodSelect instanceof MemberSelectTree memberSelect) {
            return memberSelect.getIdentifier().toString();
        }
        return methodSelect.toString();
    }

    private static void rejectTestResetCall(
        String methodName,
        String sourceIdentity,
        String enclosingClass,
        String enclosingMethod,
        String expression,
        String sourceFqcn,
        Set<String> externalStaticResetMethods,
        boolean wildcardStaticResetImport
    ) {
        if (!TEST_RESET_METHODS.contains(methodName)) {
            return;
        }
        boolean sourceResetReference = expression.startsWith(enclosingClass + "::")
            || expression.startsWith(enclosingClass + ".")
            || expression.startsWith(sourceFqcn + "::")
            || expression.startsWith(sourceFqcn + ".")
            || (!expression.contains(".") && !expression.contains("::"));
        boolean declarationContext = enclosingMethod != null
            && TEST_RESET_METHODS.contains(enclosingMethod)
            && sourceResetReference;
        boolean forbidden = externalStaticResetMethods.contains(methodName)
            || wildcardStaticResetImport
            || !declarationContext;
        if (forbidden) {
            throw new AssertionError(
                sourceIdentity + " 的 production source 不得从 "
                    + (enclosingMethod == null ? "字段/初始化器" : enclosingMethod)
                    + " 调用或引用 test reset " + methodName
            );
        }
    }

    public static void assertMethodUsesOnlyAllowedCallsAndNoStoreReferences(
        String source,
        String className,
        String methodName,
        Set<String> storeFqcns,
        Set<String> allowedInvocations,
        Set<String> allowedMemberReferences
    ) {
        CompilationUnitTree unit = parse(className, source);
        ClassTree owner = unit.getTypeDecls().stream()
            .filter(ClassTree.class::isInstance)
            .map(ClassTree.class::cast)
            .filter(type -> type.getSimpleName().contentEquals(className))
            .findFirst()
            .orElseThrow(() -> new AssertionError("无法定位 production 类型：" + className));
        List<MethodTree> methods = owner.getMembers().stream()
            .filter(MethodTree.class::isInstance)
            .map(MethodTree.class::cast)
            .filter(method -> method.getName().contentEquals(methodName))
            .toList();
        if (methods.size() != 1) {
            throw new AssertionError("production helper 必须恰好声明一次：" + className + "." + methodName);
        }
        Set<String> importedStoreTypeNames = new TreeSet<>();
        Set<String> staticImportedStoreMembers = new TreeSet<>();
        String sourcePackage = unit.getPackageName() == null ? "" : unit.getPackageName().toString();
        for (String fqcn : storeFqcns) {
            if (packageName(fqcn).equals(sourcePackage)) importedStoreTypeNames.add(simpleName(fqcn));
        }
        for (ImportTree importTree : unit.getImports()) {
            String imported = importTree.getQualifiedIdentifier().toString();
            for (String fqcn : storeFqcns) {
                if (!importTree.isStatic() && imported.equals(fqcn)) importedStoreTypeNames.add(simpleName(fqcn));
                if (importTree.isStatic() && imported.startsWith(fqcn + ".")) {
                    String member = imported.substring(fqcn.length() + 1);
                    if (!member.equals("*")) staticImportedStoreMembers.add(member);
                }
            }
        }
        Set<String> rejectedCalls = new TreeSet<>();
        Set<String> rejectedStoreReferences = new TreeSet<>();
        new TreeScanner<Void, Void>() {
            private void rejectQualifiedStoreReference(String expression) {
                for (String fqcn : storeFqcns) {
                    if (expression.equals(fqcn) || expression.startsWith(fqcn + ".")) rejectedStoreReferences.add(expression);
                }
            }
            @Override public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                String call = invocation.getMethodSelect().toString();
                if (!allowedInvocations.contains(call)) rejectedCalls.add("invoke:" + call);
                if (invocation.getMethodSelect() instanceof IdentifierTree identifier
                    && staticImportedStoreMembers.contains(identifier.getName().toString())) {
                    rejectedStoreReferences.add("static-import:" + identifier.getName());
                }
                return super.visitMethodInvocation(invocation, unused);
            }
            @Override public Void visitMemberReference(MemberReferenceTree reference, Void unused) {
                String memberReference = reference.getQualifierExpression() + "::" + reference.getName();
                if (!allowedMemberReferences.contains(memberReference)) rejectedCalls.add("reference:" + memberReference);
                rejectQualifiedStoreReference(reference.getQualifierExpression().toString());
                return super.visitMemberReference(reference, unused);
            }
            @Override public Void visitNewClass(NewClassTree expression, Void unused) {
                rejectedCalls.add("new:" + expression.getIdentifier());
                return super.visitNewClass(expression, unused);
            }
            @Override public Void visitIdentifier(IdentifierTree identifier, Void unused) {
                if (importedStoreTypeNames.contains(identifier.getName().toString())) rejectedStoreReferences.add(identifier.getName().toString());
                return super.visitIdentifier(identifier, unused);
            }
            @Override public Void visitMemberSelect(MemberSelectTree selection, Void unused) {
                rejectQualifiedStoreReference(selection.toString());
                return super.visitMemberSelect(selection, unused);
            }
        }.scan(methods.get(0).getBody(), null);
        if (!rejectedCalls.isEmpty() || !rejectedStoreReferences.isEmpty()) {
            throw new AssertionError(className + "." + methodName
                + " 只能调用显式 allowlist 中的断线清理；未授权调用=" + rejectedCalls
                + "，Store 引用=" + rejectedStoreReferences);
        }
    }

    private static String rawTypeName(String declaredType) {
        int genericStart = declaredType.indexOf('<');
        String raw = genericStart >= 0 ? declaredType.substring(0, genericStart) : declaredType;
        int arrayStart = raw.indexOf('[');
        return arrayStart >= 0 ? raw.substring(0, arrayStart) : raw;
    }

    private static CompilationUnitTree parse(String simpleName, String source) {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        if (compiler == null) {
            throw new AssertionError("source enforcement 必须运行在完整 JDK 上");
        }
        DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
        JavaFileObject sourceFile = new StringSource(simpleName, source);
        JavacTask task = (JavacTask) compiler.getTask(
            null,
            null,
            diagnostics,
            List.of("-proc:none"),
            null,
            List.of(sourceFile)
        );
        try {
            CompilationUnitTree unit = task.parse().iterator().next();
            List<String> errors = diagnostics.getDiagnostics().stream()
                .filter(diagnostic -> diagnostic.getKind() == Diagnostic.Kind.ERROR)
                .map(Diagnostic::toString)
                .toList();
            if (!errors.isEmpty()) {
                throw new AssertionError("无法解析 Java source：" + errors);
            }
            return unit;
        } catch (IOException exception) {
            throw new AssertionError("无法解析 Java source", exception);
        }
    }

    private static String packageName(String identity) {
        int separator = identity.lastIndexOf('.');
        return separator >= 0 ? identity.substring(0, separator) : "";
    }

    private static String simpleName(String identity) {
        int separator = identity.lastIndexOf('.');
        return separator >= 0 ? identity.substring(separator + 1) : identity;
    }

    private static final class StringSource extends SimpleJavaFileObject {
        private final String source;

        private StringSource(String simpleName, String source) {
            super(URI.create("string:///" + simpleName + Kind.SOURCE.extension), Kind.SOURCE);
            this.source = source;
        }

        @Override
        public CharSequence getCharContent(boolean ignoreEncodingErrors) {
            return source;
        }
    }
}
