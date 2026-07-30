package com.bong.client.lifecycle;

import com.sun.source.tree.ClassTree;
import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.IdentifierTree;
import com.sun.source.tree.ImportTree;
import com.sun.source.tree.LambdaExpressionTree;
import com.sun.source.tree.MemberReferenceTree;
import com.sun.source.tree.MemberSelectTree;
import com.sun.source.tree.MethodInvocationTree;
import com.sun.source.tree.MethodTree;
import com.sun.source.tree.NewClassTree;
import com.sun.source.tree.PrimitiveTypeTree;
import com.sun.source.tree.Tree;
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
    private static final Set<String> STORE_CLEANUP_METHODS = Set.of(
        "clearOnDisconnect",
        "clear",
        "clearAll",
        "reset"
    );
    private static final String DISCONNECT_CLEANER_METHOD = "clearOnDisconnect";

    public record AuditedLifecycleEntry(
        String sourceIdentity,
        String source,
        String className,
        String methodName,
        int parameterCount
    ) {
        public AuditedLifecycleEntry(
            String sourceIdentity,
            String source,
            String className,
            String methodName
        ) {
            this(sourceIdentity, source, className, methodName, 0);
        }
    }

    record StoreRegistration(String storeType, String cleanerOwner, String cleanerMethod) {}

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
        CompilationUnitTree unit = parse(simpleName(sourceIdentity), source);
        new TreeScanner<Void, Void>() {
            private String enclosingMethod;

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
                    enclosingMethod
                );
                return super.visitMethodInvocation(invocation, unused);
            }

            @Override
            public Void visitMemberReference(MemberReferenceTree reference, Void unused) {
                rejectTestResetCall(
                    reference.getName().toString(),
                    sourceIdentity,
                    enclosingMethod
                );
                return super.visitMemberReference(reference, unused);
            }
        }.scan(unit, null);
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
            throw new AssertionError(
                "production helper 必须恰好声明一次：" + className + "." + methodName
                    + "；实际=" + methods.size()
            );
        }

        Set<String> importedStoreTypeNames = new TreeSet<>();
        Set<String> staticImportedStoreMembers = new TreeSet<>();
        String sourcePackage = unit.getPackageName() == null ? "" : unit.getPackageName().toString();
        for (String fqcn : storeFqcns) {
            if (packageName(fqcn).equals(sourcePackage)) {
                importedStoreTypeNames.add(simpleName(fqcn));
            }
        }
        for (ImportTree importTree : unit.getImports()) {
            String imported = importTree.getQualifiedIdentifier().toString();
            for (String fqcn : storeFqcns) {
                if (!importTree.isStatic() && imported.equals(fqcn)) {
                    importedStoreTypeNames.add(simpleName(fqcn));
                }
                if (importTree.isStatic() && imported.startsWith(fqcn + ".")) {
                    String member = imported.substring(fqcn.length() + 1);
                    if (!member.equals("*")) {
                        staticImportedStoreMembers.add(member);
                    }
                }
            }
        }

        Set<String> rejectedCalls = new TreeSet<>();
        Set<String> rejectedStoreReferences = new TreeSet<>();
        new TreeScanner<Void, Void>() {
            private void rejectQualifiedStoreReference(String expression) {
                for (String fqcn : storeFqcns) {
                    if (expression.equals(fqcn) || expression.startsWith(fqcn + ".")) {
                        rejectedStoreReferences.add(expression);
                    }
                }
            }

            @Override
            public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                String call = invocation.getMethodSelect().toString();
                if (!allowedInvocations.contains(call)) {
                    rejectedCalls.add("invoke:" + call);
                }
                if (invocation.getMethodSelect() instanceof IdentifierTree identifier
                    && staticImportedStoreMembers.contains(identifier.getName().toString())) {
                    rejectedStoreReferences.add("static-import:" + identifier.getName());
                }
                return super.visitMethodInvocation(invocation, unused);
            }

            @Override
            public Void visitMemberReference(MemberReferenceTree reference, Void unused) {
                String memberReference = reference.getQualifierExpression()
                    + "::" + reference.getName();
                if (!allowedMemberReferences.contains(memberReference)) {
                    rejectedCalls.add("reference:" + memberReference);
                }
                rejectQualifiedStoreReference(reference.getQualifierExpression().toString());
                return super.visitMemberReference(reference, unused);
            }

            @Override
            public Void visitNewClass(NewClassTree expression, Void unused) {
                rejectedCalls.add("new:" + expression.getIdentifier());
                return super.visitNewClass(expression, unused);
            }

            @Override
            public Void visitIdentifier(IdentifierTree identifier, Void unused) {
                if (importedStoreTypeNames.contains(identifier.getName().toString())) {
                    rejectedStoreReferences.add(identifier.getName().toString());
                }
                return super.visitIdentifier(identifier, unused);
            }

            @Override
            public Void visitMemberSelect(MemberSelectTree selection, Void unused) {
                rejectQualifiedStoreReference(selection.toString());
                return super.visitMemberSelect(selection, unused);
            }
        }.scan(methods.get(0).getBody(), null);

        if (!rejectedCalls.isEmpty() || !rejectedStoreReferences.isEmpty()) {
            throw new AssertionError(
                className + "." + methodName
                    + " 只能调用显式 allowlist 中的断线清理；未授权调用=" + rejectedCalls
                    + "，Store 引用=" + rejectedStoreReferences
            );
        }
    }

    public static void assertMethodContainsNoStoreReferences(
        String source,
        String className,
        String methodName,
        Set<String> storeFqcns
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
            throw new AssertionError(
                "production helper 必须恰好声明一次：" + className + "." + methodName
                    + "；实际=" + methods.size()
            );
        }

        String sourcePackage = unit.getPackageName() == null ? "" : unit.getPackageName().toString();
        Set<String> visibleStoreTypeNames = new TreeSet<>();
        Set<String> staticImportedStoreMemberNames = new TreeSet<>();
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
                if (importTree.isStatic() && imported.startsWith(fqcn + ".")) {
                    String member = imported.substring(fqcn.length() + 1);
                    if (!member.equals("*")) {
                        staticImportedStoreMemberNames.add(member);
                    }
                }
            }
        }

        Set<String> references = new TreeSet<>();
        new TreeScanner<Void, Void>() {
            @Override
            public Void visitIdentifier(IdentifierTree identifier, Void unused) {
                String identifierName = identifier.getName().toString();
                if (visibleStoreTypeNames.contains(identifierName)) {
                    references.add(identifierName);
                }
                if (staticImportedStoreMemberNames.contains(identifierName)) {
                    references.add("static-import:" + identifierName);
                }
                return super.visitIdentifier(identifier, unused);
            }

            @Override
            public Void visitMemberSelect(MemberSelectTree selection, Void unused) {
                for (String fqcn : storeFqcns) {
                    String expression = selection.toString();
                    if (expression.equals(fqcn) || expression.startsWith(fqcn + ".")) {
                        references.add(expression);
                    }
                }
                return super.visitMemberSelect(selection, unused);
            }
        }.scan(methods.get(0).getBody(), null);

        if (!references.isEmpty()) {
            throw new AssertionError(
                className + "." + methodName
                    + " 不得直接拥有 registry-managed Store 引用：" + references
            );
        }
    }

    public static void assertLifecycleClosureContainsNoStoreCleanupReferences(
        List<AuditedLifecycleEntry> entries,
        Set<String> storeFqcns
    ) {
        Map<String, AuditedLifecycleEntry> entriesByCall = new HashMap<>();
        for (AuditedLifecycleEntry entry : entries) {
            String call = lifecycleEntryCall(entry);
            AuditedLifecycleEntry previous = entriesByCall.put(call, entry);
            if (previous != null) {
                throw new AssertionError("audited lifecycle closure 重复登记：" + call);
            }
        }

        Set<String> auditedCalls = Set.copyOf(entriesByCall.keySet());
        List<String> violations = new ArrayList<>();
        for (AuditedLifecycleEntry entry : entries) {
            try {
                assertMethodClosureContainsNoStoreCleanupReferences(
                    entry.source(),
                    entry.className(),
                    entry.methodName(),
                    entry.parameterCount(),
                    storeFqcns,
                    auditedCalls
                );
            } catch (AssertionError failure) {
                violations.add(failure.getMessage());
            }
        }
        if (!violations.isEmpty()) {
            throw new AssertionError("audited lifecycle closure 校验失败：" + violations);
        }
    }

    public static void assertMethodClosureContainsNoStoreCleanupReferences(
        String source,
        String className,
        String methodName,
        Set<String> storeFqcns,
        Set<String> allowedCrossClassCalls
    ) {
        assertMethodClosureContainsNoStoreCleanupReferences(
            source,
            className,
            methodName,
            0,
            storeFqcns,
            allowedCrossClassCalls
        );
    }

    private static void assertMethodClosureContainsNoStoreCleanupReferences(
        String source,
        String className,
        String methodName,
        int parameterCount,
        Set<String> storeFqcns,
        Set<String> allowedCrossClassCalls
    ) {
        CompilationUnitTree unit = parse(className, source);
        ClassTree owner = findDeclaredType(unit, simpleName(className))
            .orElseThrow(() -> new AssertionError("无法定位 production 类型：" + className));
        Map<String, List<MethodTree>> methodsByName = new HashMap<>();
        for (Tree member : owner.getMembers()) {
            if (member instanceof MethodTree method) {
                methodsByName.computeIfAbsent(method.getName().toString(), ignored -> new ArrayList<>())
                    .add(method);
            }
        }
        Map<String, String> currentClassFieldTypes = new HashMap<>();
        Map<String, String> currentClassFieldElementTypes = new HashMap<>();
        for (Tree member : owner.getMembers()) {
            if (member instanceof VariableTree variable && variable.getType() != null) {
                String declaredType = variable.getType().toString();
                currentClassFieldTypes.put(
                    variable.getName().toString(),
                    rawTypeName(declaredType)
                );
                String elementType = singleGenericTypeArgument(declaredType);
                if (elementType != null) {
                    currentClassFieldElementTypes.put(variable.getName().toString(), elementType);
                }
            }
        }
        Map<String, Map<String, String>> localDataAccessors = new HashMap<>();
        collectLocalDataAccessors(unit, localDataAccessors);
        List<MethodTree> entries = methodsByName.getOrDefault(methodName, List.of()).stream()
            .filter(method -> method.getParameters().size() == parameterCount)
            .toList();
        if (entries.isEmpty()
            && owner.getKind() == Tree.Kind.RECORD
            && localDataAccessors.getOrDefault(className, Map.of()).containsKey(methodName)
            && parameterCount == 0) {
            return;
        }
        if (entries.size() != 1) {
            throw new AssertionError(
                "production helper 必须恰好声明一个匹配入口：" + className + "." + methodName
                    + "/" + parameterCount + "；实际=" + entries.size()
            );
        }
        Map<String, String> visibleTypeFqcns = new HashMap<>();
        String sourcePackage = unit.getPackageName() == null ? "" : unit.getPackageName().toString();
        for (Tree declaration : unit.getTypeDecls()) {
            if (declaration instanceof ClassTree type) {
                registerVisibleDeclaredTypes(type, sourcePackage, null, visibleTypeFqcns);
            }
        }
        Set<String> visibleStoreTypeNames = new TreeSet<>();
        Set<String> staticallyImportedCleanupMethods = new TreeSet<>();
        Set<String> violations = new TreeSet<>();
        for (String fqcn : storeFqcns) {
            if (packageName(fqcn).equals(sourcePackage)) {
                visibleStoreTypeNames.add(simpleName(fqcn));
            }
        }
        for (ImportTree importTree : unit.getImports()) {
            String imported = importTree.getQualifiedIdentifier().toString();
            if (!importTree.isStatic() && !imported.endsWith(".*")) {
                visibleTypeFqcns.put(simpleName(imported), imported);
                collectImportedDataAccessors(imported, localDataAccessors);
            }
            for (String fqcn : storeFqcns) {
                if (!importTree.isStatic() && imported.equals(fqcn)) {
                    visibleStoreTypeNames.add(simpleName(fqcn));
                }
                if (!importTree.isStatic() && imported.equals(packageName(fqcn) + ".*")) {
                    violations.add("wildcard-import:" + imported);
                }
                if (importTree.isStatic() && imported.equals(fqcn + ".*")) {
                    violations.add("wildcard-import:static " + imported);
                }
                if (importTree.isStatic()
                    && STORE_CLEANUP_METHODS.stream()
                        .anyMatch(cleanup -> imported.equals(fqcn + "." + cleanup))) {
                    staticallyImportedCleanupMethods.add(
                        imported.substring(imported.lastIndexOf('.') + 1)
                    );
                }
            }
        }

        Set<String> visited = new HashSet<>();
        com.sun.source.util.TreePath methodPath = com.sun.source.util.TreePath.getPath(unit, entries.get(0));
        if (methodPath == null) {
            throw new AssertionError("无法定位 production helper AST path：" + className + "." + methodName);
        }
        scanMethodClosure(
            entries.get(0),
            methodPath,
            className,
            methodsByName,
            visited,
            storeFqcns,
            visibleStoreTypeNames,
            currentClassFieldTypes,
            currentClassFieldElementTypes,
            localDataAccessors,
            visibleTypeFqcns,
            sourcePackage,
            staticallyImportedCleanupMethods,
            allowedCrossClassCalls,
            violations
        );
        if (!violations.isEmpty()) {
            throw new AssertionError(
                className + "." + methodName
                    + " 的可达断线清理 closure 不得触达 registry-managed Store：" + violations
            );
        }
    }

    private static void scanMethodClosure(
        MethodTree method,
        com.sun.source.util.TreePath methodPath,
        String className,
        Map<String, List<MethodTree>> methodsByName,
        Set<String> visited,
        Set<String> storeFqcns,
        Set<String> visibleStoreTypeNames,
        Map<String, String> currentClassFieldTypes,
        Map<String, String> currentClassFieldElementTypes,
        Map<String, Map<String, String>> localDataAccessors,
        Map<String, String> visibleTypeFqcns,
        String sourcePackage,
        Set<String> staticallyImportedCleanupMethods,
        Set<String> allowedCrossClassCalls,
        Set<String> violations
    ) {
        String methodKey = method.getName() + "/" + method.getParameters().size();
        if (!visited.add(methodKey) || method.getBody() == null) {
            return;
        }
        Map<String, String> localDataVariables = new HashMap<>();
        for (VariableTree parameter : method.getParameters()) {
            if (parameter.getType() != null) {
                localDataVariables.put(
                    parameter.getName().toString(),
                    rawTypeName(parameter.getType().toString())
                );
            }
        }
        new TreePathScanner<Void, Void>() {
            @Override
            public Void visitVariable(VariableTree variable, Void unused) {
                if (variable.getType() != null) {
                    localDataVariables.put(
                        variable.getName().toString(),
                        rawTypeName(variable.getType().toString())
                    );
                }
                return super.visitVariable(variable, unused);
            }

            @Override
            public Void visitLambdaExpression(LambdaExpressionTree expression, Void unused) {
                if (expression.getParameters().size() != 1) {
                    return super.visitLambdaExpression(expression, unused);
                }
                String parameterName = expression.getParameters().get(0).getName().toString();
                String inferredType = inferLambdaParameterType(
                    getCurrentPath(),
                    currentClassFieldElementTypes
                );
                if (inferredType == null) {
                    return super.visitLambdaExpression(expression, unused);
                }
                String previousType = localDataVariables.put(parameterName, inferredType);
                try {
                    return super.visitLambdaExpression(expression, unused);
                } finally {
                    if (previousType == null) {
                        localDataVariables.remove(parameterName);
                    } else {
                        localDataVariables.put(parameterName, previousType);
                    }
                }
            }

            @Override
            public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                String invokedName = invokedMethodName(invocation.getMethodSelect());
                if (invocation.getMethodSelect() instanceof MemberSelectTree selection) {
                    String owner = selection.getExpression().toString();
                    String resolvedOwner = resolveReceiverType(
                        owner,
                        currentClassFieldTypes,
                        localDataVariables
                    );
                    if (isManagedStoreOwner(resolvedOwner, storeFqcns, visibleStoreTypeNames)) {
                        violations.add("store-receiver:" + invocation.getMethodSelect());
                    } else if (owner.equals("this") || owner.equals(className) || resolvedOwner.equals(className)) {
                        recurseLocal(invokedName, invocation.getArguments().size());
                    } else {
                        String crossClassCall = normalizeCrossClassCall(
                            owner,
                            invokedName,
                            invocation.getArguments().size(),
                            currentClassFieldTypes,
                            localDataVariables,
                            localDataAccessors,
                            visibleTypeFqcns,
                            sourcePackage
                        );
                        if (crossClassCall != null
                            && !isAllowedCrossClassCall(crossClassCall, allowedCrossClassCalls)) {
                            violations.add("cross-class:" + crossClassCall);
                        }
                    }
                } else if (invocation.getMethodSelect() instanceof IdentifierTree) {
                    if (STORE_CLEANUP_METHODS.contains(invokedName)
                        && staticallyImportedCleanupMethods.contains(invokedName)) {
                        violations.add("static-import:" + invokedName);
                    } else {
                        recurseLocal(invokedName, invocation.getArguments().size());
                    }
                }
                return super.visitMethodInvocation(invocation, unused);
            }

            @Override
            public Void visitMemberReference(MemberReferenceTree reference, Void unused) {
                String referencedName = reference.getName().toString();
                String qualifier = reference.getQualifierExpression().toString();
                String resolvedQualifier = resolveReceiverType(
                    qualifier,
                    currentClassFieldTypes,
                    localDataVariables
                );
                int referenceArity = memberReferenceTargetArity(
                    qualifier,
                    referencedName,
                    currentClassFieldTypes,
                    localDataVariables,
                    localDataAccessors,
                    visibleTypeFqcns,
                    sourcePackage,
                    allowedCrossClassCalls
                );
                if (isManagedStoreOwner(resolvedQualifier, storeFqcns, visibleStoreTypeNames)) {
                    violations.add("store-receiver-reference:" + qualifier + "::" + referencedName);
                } else {
                    String crossClassCall = normalizeCrossClassCall(
                        qualifier,
                        referencedName,
                        referenceArity,
                        currentClassFieldTypes,
                        localDataVariables,
                        localDataAccessors,
                        visibleTypeFqcns,
                        sourcePackage
                    );
                    if (crossClassCall != null
                        && !isAllowedCrossClassCall(crossClassCall, allowedCrossClassCalls)) {
                        violations.add("cross-class-reference:" + crossClassCall);
                    }
                }
                return super.visitMemberReference(reference, unused);
            }

            @Override
            public Void visitNewClass(NewClassTree expression, Void unused) {
                String identifier = rawTypeName(expression.getIdentifier().toString());
                String constructorFqcn = visibleTypeFqcns.getOrDefault(identifier, identifier);
                if (isManagedStoreOwner(constructorFqcn, storeFqcns, visibleStoreTypeNames)) {
                    violations.add("new-store:" + expression.getIdentifier());
                }
                return super.visitNewClass(expression, unused);
            }

            private void recurseLocal(String name, int parameterCount) {
                List<MethodTree> candidates = methodsByName.getOrDefault(name, List.of()).stream()
                    .filter(candidate -> candidate.getParameters().size() == parameterCount)
                    .toList();
                if (candidates.size() > 1) {
                    violations.add("ambiguous-local-helper:" + name + "/" + parameterCount);
                    return;
                }
                if (candidates.size() == 1) {
                    com.sun.source.util.TreePath candidatePath = com.sun.source.util.TreePath.getPath(
                        methodPath.getCompilationUnit(),
                        candidates.get(0)
                    );
                    if (candidatePath == null) {
                        violations.add("unresolved-local-helper-path:" + name + "/" + parameterCount);
                        return;
                    }
                    scanMethodClosure(
                        candidates.get(0),
                        candidatePath,
                        className,
                        methodsByName,
                        visited,
                        storeFqcns,
                        visibleStoreTypeNames,
                        currentClassFieldTypes,
                        currentClassFieldElementTypes,
                        localDataAccessors,
                        visibleTypeFqcns,
                        sourcePackage,
                        staticallyImportedCleanupMethods,
                        allowedCrossClassCalls,
                        violations
                    );
                }
            }
        }.scan(methodPath, null);
    }

    private static int memberReferenceTargetArity(
        String qualifier,
        String methodName,
        Map<String, String> currentClassFieldTypes,
        Map<String, String> localDataVariables,
        Map<String, Map<String, String>> localDataAccessors,
        Map<String, String> visibleTypeFqcns,
        String sourcePackage,
        Set<String> allowedCrossClassCalls
    ) {
        String receiverType = resolveReceiverType(qualifier, currentClassFieldTypes, localDataVariables);
        String targetType = localDataAccessors.getOrDefault(receiverType, Map.of()).get(methodName);
        if (targetType != null) {
            return Math.max(0, functionalInterfaceArity(targetType));
        }
        String resolvedReceiver = visibleTypeFqcns.getOrDefault(receiverType, receiverType);
        if (resolvedReceiver.equals(receiverType)
            && !receiverType.contains(".")
            && isCrossClassStaticCallOwner(receiverType)) {
            resolvedReceiver = sourcePackage.isBlank()
                ? receiverType
                : sourcePackage + "." + receiverType;
        }
        String prefix = resolvedReceiver + "." + methodName + "/";
        List<Integer> allowedArities = allowedCrossClassCalls.stream()
            .filter(call -> call.startsWith(prefix))
            .map(call -> call.substring(prefix.length()))
            .map(Integer::parseInt)
            .toList();
        if (allowedArities.size() == 1) {
            return allowedArities.get(0);
        }
        return 0;
    }

    private static int functionalInterfaceArity(String ownerType) {
        return switch (ownerType) {
            case "Runnable", "Supplier", "BooleanSupplier", "IntSupplier", "LongSupplier", "DoubleSupplier" -> 0;
            case "Consumer", "Predicate", "Function", "UnaryOperator" -> 1;
            case "BiConsumer", "BiPredicate", "BiFunction", "BinaryOperator" -> 2;
            default -> -1;
        };
    }

    private static String resolveReceiverType(
        String owner,
        Map<String, String> currentClassFieldTypes,
        Map<String, String> localDataVariables
    ) {
        String ownerType = currentClassFieldTypes.get(owner);
        if (ownerType == null) {
            ownerType = localDataVariables.get(owner);
        }
        return ownerType == null ? owner : ownerType;
    }

    private static boolean isAllowedCrossClassCall(
        String crossClassCall,
        Set<String> allowedCrossClassCalls
    ) {
        return allowedCrossClassCalls.contains(crossClassCall);
    }

    private static String lifecycleEntryCall(AuditedLifecycleEntry entry) {
        CompilationUnitTree unit = parse(entry.className(), entry.source());
        String packageName = unit.getPackageName() == null ? "" : unit.getPackageName().toString();
        String call = packageName.isBlank()
            ? entry.className() + "." + entry.methodName()
            : packageName + "." + entry.className() + "." + entry.methodName();
        return call + "/" + entry.parameterCount();
    }

    private static String normalizeCrossClassCall(
        String owner,
        String methodName,
        int parameterCount,
        Map<String, String> currentClassFieldTypes,
        Map<String, String> localDataVariables,
        Map<String, Map<String, String>> localDataAccessors,
        Map<String, String> visibleTypeFqcns,
        String sourcePackage
    ) {
        String ownerType = currentClassFieldTypes.get(owner);
        if (ownerType == null) {
            ownerType = localDataVariables.get(owner);
        }
        if (ownerType == null) {
            String rootOwner = rootOwner(owner);
            if (!rootOwner.equals(owner)) {
                String rootType = currentClassFieldTypes.get(rootOwner);
                if (rootType == null) {
                    rootType = localDataVariables.get(rootOwner);
                }
                if (rootType != null) {
                    String resolvedChainType = resolveLocalDataChainType(
                        owner,
                        currentClassFieldTypes,
                        localDataVariables,
                        localDataAccessors
                    );
                    if (resolvedChainType != null) {
                        return isFrameworkDataType(visibleTypeFqcns.getOrDefault(
                            resolvedChainType,
                            resolvedChainType
                        )) || isStandardLibraryDataCall(resolvedChainType, methodName)
                            ? null
                            : unresolvedCrossClassCall(owner, methodName, parameterCount);
                    }
                    String rootFqcn = visibleTypeFqcns.getOrDefault(rootType, rootType);
                    return isFrameworkDataType(rootFqcn)
                        ? null
                        : unresolvedCrossClassCall(owner, methodName, parameterCount);
                }
                String resolvedChainType = resolveLocalDataChainType(
                    owner,
                    currentClassFieldTypes,
                    localDataVariables,
                    localDataAccessors
                );
                if (resolvedChainType != null) {
                    return isFrameworkDataType(visibleTypeFqcns.getOrDefault(
                        resolvedChainType,
                        resolvedChainType
                    )) || isStandardLibraryDataCall(resolvedChainType, methodName)
                        ? null
                        : unresolvedCrossClassCall(owner, methodName, parameterCount);
                }
            }
            ownerType = owner;
        }
        if (isLocalDataCall(ownerType, methodName, localDataAccessors)) {
            return null;
        }
        int chainedCall = ownerType.indexOf('.');
        if (ownerType.endsWith("()") && chainedCall > 0) {
            ownerType = ownerType.substring(0, chainedCall);
        }
        String ownerFqcn = visibleTypeFqcns.getOrDefault(ownerType, ownerType);
        if (ownerType.equals("BongClient.LOGGER")) {
            return null;
        }
        String root = rootOwner(owner);
        String rootType = localDataVariables.get(root);
        String rootFqcn = rootType == null ? "" : visibleTypeFqcns.getOrDefault(rootType, rootType);
        if (isFrameworkDataType(rootFqcn) || isFrameworkDataType(ownerFqcn)) {
            return null;
        }
        if (isCrossClassStaticCallOwner(ownerType)) {
            if (ownerFqcn.equals(ownerType) && !ownerType.contains(".")) {
                ownerFqcn = sourcePackage.isBlank() ? ownerType : sourcePackage + "." + ownerType;
            }
            return ownerFqcn + "." + methodName + "/" + parameterCount;
        }
        if (ownerType.matches("[a-z_$][A-Za-z0-9_$]*")) {
            return unresolvedCrossClassCall(owner, methodName, parameterCount);
        }
        return null;
    }

    private static boolean isLocalDataCall(
        String ownerType,
        String methodName,
        Map<String, Map<String, String>> localDataAccessors
    ) {
        return localDataAccessors.getOrDefault(ownerType, Map.of()).containsKey(methodName)
            || isStandardLibraryDataCall(ownerType, methodName);
    }

    private static String resolveLocalDataChainType(
        String owner,
        Map<String, String> currentClassFieldTypes,
        Map<String, String> localDataVariables,
        Map<String, Map<String, String>> localDataAccessors
    ) {
        String[] segments = owner.split("\\.");
        if (segments.length < 2) {
            return null;
        }
        String currentType = currentClassFieldTypes.get(segments[0]);
        if (currentType == null) {
            currentType = localDataVariables.get(segments[0]);
        }
        if (currentType == null) {
            return null;
        }
        for (int index = 1; index < segments.length; index++) {
            String accessor = segments[index];
            if (accessor.endsWith("()")) {
                accessor = accessor.substring(0, accessor.length() - 2);
            }
            currentType = localDataAccessors.getOrDefault(currentType, Map.of()).get(accessor);
            if (currentType == null) {
                return null;
            }
        }
        return currentType;
    }

    private static boolean isStandardLibraryDataCall(String ownerType, String methodName) {
        return switch (ownerType) {
            case "Runnable" -> methodName.equals("run");
            case "Consumer", "BiConsumer" -> methodName.equals("accept");
            case "Supplier", "BooleanSupplier", "IntSupplier", "LongSupplier", "DoubleSupplier" ->
                methodName.equals("get")
                    || methodName.equals("getAsBoolean")
                    || methodName.equals("getAsInt")
                    || methodName.equals("getAsLong")
                    || methodName.equals("getAsDouble");
            case "Predicate", "BiPredicate" -> methodName.equals("test");
            case "Function", "BiFunction", "UnaryOperator", "BinaryOperator" -> methodName.equals("apply");
            case "Optional" -> Set.of("ifPresent", "isEmpty", "isPresent", "orElse", "orElseGet", "map")
                .contains(methodName);
            default -> false;
        };
    }

    private static boolean isFrameworkDataType(String fqcn) {
        return fqcn.equals("Math")
            || fqcn.startsWith("java.")
            || fqcn.startsWith("javax.")
            || Set.of(
                "ArithmeticException",
                "Exception",
                "IllegalArgumentException",
                "IllegalStateException",
                "RuntimeException",
                "String",
                "Throwable"
            ).contains(fqcn)
            || fqcn.startsWith("net.minecraft.")
            || fqcn.startsWith("net.fabricmc.")
            || fqcn.startsWith("org.lwjgl.")
            || fqcn.startsWith("org.slf4j.")
            || fqcn.startsWith("dev.kosmx.playerAnim.");
    }

    private static String rootOwner(String owner) {
        int separator = owner.indexOf('.');
        return separator >= 0 ? owner.substring(0, separator) : owner;
    }

    private static String unresolvedCrossClassCall(
        String owner,
        String methodName,
        int parameterCount
    ) {
        return "<unresolved>:" + owner + "." + methodName + "/" + parameterCount;
    }

    private static void collectLocalDataAccessors(
        CompilationUnitTree unit,
        Map<String, Map<String, String>> localDataAccessors
    ) {
        for (Tree declaration : unit.getTypeDecls()) {
            if (declaration instanceof ClassTree type) {
                collectLocalDataAccessors(type, null, localDataAccessors);
            }
        }
    }

    private static void collectImportedDataAccessors(
        String importedFqcn,
        Map<String, Map<String, String>> localDataAccessors
    ) {
        String relativePath = importedFqcn.replace('.', '/') + ".java";
        PathSource source = findProductionSource(relativePath);
        if (source == null) {
            return;
        }
        CompilationUnitTree importedUnit = parse(simpleName(importedFqcn), source.content());
        findDeclaredType(importedUnit, simpleName(importedFqcn))
            .ifPresent(type -> collectLocalDataAccessors(type, null, localDataAccessors));
    }

    private static PathSource findProductionSource(String relativePath) {
        java.nio.file.Path workingDirectory = java.nio.file.Path.of("").toAbsolutePath().normalize();
        for (java.nio.file.Path root : List.of(
            workingDirectory.resolve("src/main/java"),
            workingDirectory.resolve("client/src/main/java")
        )) {
            java.nio.file.Path candidate = root.resolve(relativePath);
            if (java.nio.file.Files.isRegularFile(candidate)) {
                try {
                    return new PathSource(candidate, java.nio.file.Files.readString(candidate));
                } catch (IOException exception) {
                    throw new AssertionError("无法读取 imported data source：" + candidate, exception);
                }
            }
        }
        return null;
    }

    private record PathSource(java.nio.file.Path path, String content) {}

    private static void collectLocalDataAccessors(
        ClassTree type,
        String enclosingName,
        Map<String, Map<String, String>> localDataAccessors
    ) {
        String qualifiedName = enclosingName == null
            ? type.getSimpleName().toString()
            : enclosingName + "." + type.getSimpleName();
        Map<String, String> accessors = new HashMap<>();
        for (Tree member : type.getMembers()) {
            if (type.getKind() == Tree.Kind.RECORD
                && member instanceof MethodTree method
                && method.getParameters().isEmpty()
                && method.getReturnType() != null) {
                accessors.put(
                    method.getName().toString(),
                    rawTypeName(method.getReturnType().toString())
                );
            } else if (member instanceof VariableTree component
                && component.getType() != null
                && type.getKind() == Tree.Kind.RECORD) {
                accessors.put(
                    component.getName().toString(),
                    rawTypeName(component.getType().toString())
                );
            } else if (member instanceof ClassTree nestedType) {
                collectLocalDataAccessors(nestedType, qualifiedName, localDataAccessors);
            }
        }
        if (!accessors.isEmpty()) {
            Map<String, String> immutableAccessors = Map.copyOf(accessors);
            localDataAccessors.putIfAbsent(type.getSimpleName().toString(), immutableAccessors);
            localDataAccessors.put(qualifiedName, immutableAccessors);
        }
    }

    private static String inferLambdaParameterType(
        com.sun.source.util.TreePath lambdaPath,
        Map<String, String> currentClassFieldElementTypes
    ) {
        com.sun.source.util.TreePath parentPath = lambdaPath.getParentPath();
        if (parentPath == null || !(parentPath.getLeaf() instanceof MethodInvocationTree invocation)) {
            return null;
        }
        if (!(invocation.getMethodSelect() instanceof MemberSelectTree selection)
            || !selection.getIdentifier().contentEquals("removeIf")
            || !invocation.getArguments().contains(lambdaPath.getLeaf())) {
            return null;
        }
        return currentClassFieldElementTypes.get(selection.getExpression().toString());
    }

    private static String singleGenericTypeArgument(String declaredType) {
        int start = declaredType.indexOf('<');
        int end = declaredType.lastIndexOf('>');
        if (start < 0 || end <= start + 1) {
            return null;
        }
        String argument = declaredType.substring(start + 1, end).trim();
        return argument.contains(",") || argument.contains("?")
            ? null
            : rawTypeName(argument);
    }

    private static String rawTypeName(String declaredType) {
        int genericStart = declaredType.indexOf('<');
        String raw = genericStart >= 0 ? declaredType.substring(0, genericStart) : declaredType;
        int arrayStart = raw.indexOf('[');
        return arrayStart >= 0 ? raw.substring(0, arrayStart) : raw;
    }

    private static boolean isCrossClassStaticCallOwner(String owner) {
        if (owner.equals("this") || owner.equals("super") || owner.isBlank()) {
            return false;
        }
        int separator = owner.lastIndexOf('.');
        String terminalOwner = separator >= 0 ? owner.substring(separator + 1) : owner;
        return !terminalOwner.isBlank() && Character.isUpperCase(terminalOwner.charAt(0));
    }

    static void assertRegistryOwnsManagedStoreCleanerCalls(
        String source,
        String sourceIdentity,
        Set<String> storeFqcns,
        boolean registrySource
    ) {
        CompilationUnitTree unit = parse(simpleName(sourceIdentity), source);
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
                if (importTree.isStatic()
                    && imported.equals(fqcn + ".*")) {
                    forbiddenWildcardImports.add("static " + imported);
                }
                if (importTree.isStatic()
                    && imported.equals(fqcn + "." + DISCONNECT_CLEANER_METHOD)) {
                    staticallyImportedCleanupMethods.add(DISCONNECT_CLEANER_METHOD);
                }
            }
        }

        Set<String> violations = new TreeSet<>();
        forbiddenWildcardImports.forEach(value -> violations.add("wildcard-import:" + value));
        new TreePathScanner<Void, Void>() {
            private String enclosingMethod;

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
                String methodName = invokedMethodName(invocation.getMethodSelect());
                if (!methodName.equals(DISCONNECT_CLEANER_METHOD)) {
                    return super.visitMethodInvocation(invocation, unused);
                }
                if (invocation.getMethodSelect() instanceof MemberSelectTree selection) {
                    String owner = selection.getExpression().toString();
                    if (isForbiddenManagedStoreCall(
                        owner,
                        storeFqcns,
                        visibleStoreTypeNames
                    )) {
                        violations.add("invoke:" + invocation.getMethodSelect());
                    }
                } else if (invocation.getMethodSelect() instanceof IdentifierTree) {
                    if (staticallyImportedCleanupMethods.contains(methodName)) {
                        violations.add("static-import:" + methodName);
                    } else if (sourceIsManagedStore && methodName.equals("clearOnDisconnect")) {
                        // Store 可以在 canonical wrapper / test reset 内委托 legacy/data-only helper；
                        // 但任何方法反向调用 clearOnDisconnect 都会绕过 registry 唯一所有权。
                        violations.add("self-invoke:" + methodName);
                    }
                }
                return super.visitMethodInvocation(invocation, unused);
            }

            @Override
            public Void visitMemberReference(MemberReferenceTree reference, Void unused) {
                if (!reference.getName().contentEquals(DISCONNECT_CLEANER_METHOD)
                    || !isManagedStoreOwner(
                        reference.getQualifierExpression().toString(),
                        storeFqcns,
                        visibleStoreTypeNames
                    )) {
                    return super.visitMemberReference(reference, unused);
                }
                Tree parent = getCurrentPath().getParentPath().getLeaf();
                if (!registrySource || !isSanctionedRegistryBinding(reference, parent)) {
                    violations.add(
                        "reference:" + reference.getQualifierExpression() + "::" + reference.getName()
                    );
                }
                return super.visitMemberReference(reference, unused);
            }
        }.scan(unit, null);

        if (!violations.isEmpty()) {
            throw new AssertionError(
                sourceIdentity + " 不得绕过 SessionScopedStoreRegistry 清理 registry-managed Store："
                    + violations
            );
        }
    }

    private static boolean isSanctionedRegistryBinding(
        MemberReferenceTree reference,
        Tree parent
    ) {
        if (!reference.getName().contentEquals("clearOnDisconnect")
            || !(parent instanceof MethodInvocationTree invocation)
            || !invocation.getMethodSelect().toString().equals("SessionStoreHandle.forStore")
            || invocation.getArguments().size() != 2
            || invocation.getArguments().get(1) != reference
            || !(invocation.getArguments().get(0) instanceof MemberSelectTree classToken)
            || !classToken.getIdentifier().contentEquals("class")) {
            return false;
        }
        return classToken.getExpression().toString()
            .equals(reference.getQualifierExpression().toString());
    }

    private static boolean isForbiddenManagedStoreCall(
        String owner,
        Set<String> storeFqcns,
        Set<String> visibleStoreTypeNames
    ) {
        return isManagedStoreOwner(owner, storeFqcns, visibleStoreTypeNames);
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
        String enclosingMethod
    ) {
        if (TEST_RESET_METHODS.contains(methodName)
            && (enclosingMethod == null || !TEST_RESET_METHODS.contains(enclosingMethod))) {
            throw new AssertionError(
                sourceIdentity + " 的 production source 不得从 "
                    + (enclosingMethod == null ? "字段/初始化器" : enclosingMethod)
                    + " 调用或引用 test reset " + methodName
            );
        }
    }

    private static java.util.Optional<ClassTree> findDeclaredType(
        CompilationUnitTree unit,
        String className
    ) {
        java.util.ArrayDeque<ClassTree> pending = new java.util.ArrayDeque<>();
        for (Tree declaration : unit.getTypeDecls()) {
            if (declaration instanceof ClassTree type) {
                pending.add(type);
            }
        }
        while (!pending.isEmpty()) {
            ClassTree type = pending.removeFirst();
            if (type.getSimpleName().contentEquals(className)) {
                return java.util.Optional.of(type);
            }
            for (Tree member : type.getMembers()) {
                if (member instanceof ClassTree nested) {
                    pending.addLast(nested);
                }
            }
        }
        return java.util.Optional.empty();
    }

    private static void registerVisibleDeclaredTypes(
        ClassTree type,
        String sourcePackage,
        String enclosingName,
        Map<String, String> visibleTypeFqcns
    ) {
        String qualifiedName = enclosingName == null
            ? type.getSimpleName().toString()
            : enclosingName + "." + type.getSimpleName();
        String fqcn = sourcePackage.isBlank()
            ? qualifiedName
            : sourcePackage + "." + qualifiedName;
        visibleTypeFqcns.putIfAbsent(type.getSimpleName().toString(), fqcn);
        visibleTypeFqcns.put(qualifiedName, fqcn);
        for (Tree member : type.getMembers()) {
            if (member instanceof ClassTree nested) {
                registerVisibleDeclaredTypes(nested, sourcePackage, qualifiedName, visibleTypeFqcns);
            }
        }
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
