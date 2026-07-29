package com.bong.client.lifecycle;

import com.sun.source.tree.ClassTree;
import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.IdentifierTree;
import com.sun.source.tree.ImportTree;
import com.sun.source.tree.MemberReferenceTree;
import com.sun.source.tree.MemberSelectTree;
import com.sun.source.tree.MethodInvocationTree;
import com.sun.source.tree.MethodTree;
import com.sun.source.tree.NewClassTree;
import com.sun.source.tree.PrimitiveTypeTree;
import com.sun.source.tree.Tree;
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
import java.util.List;
import java.util.Set;
import java.util.TreeSet;

public final class JavaLifecycleSourceInspector {
    private static final Set<String> TEST_RESET_METHODS = Set.of(
        "resetForTests",
        "resetForTest",
        "clearForTests"
    );

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
        Set<String> managedStoreTypeNames = storeFqcns.stream()
            .map(JavaLifecycleSourceInspector::simpleName)
            .collect(java.util.stream.Collectors.toCollection(TreeSet::new));
        Set<String> visibleStoreTypeNames = new TreeSet<>();
        Set<String> staticallyImportedCleaners = new TreeSet<>();
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
                if (importTree.isStatic()
                    && (imported.equals(fqcn + ".clearOnDisconnect")
                        || imported.equals(fqcn + ".*"))) {
                    staticallyImportedCleaners.add(fqcn);
                }
            }
        }

        Set<String> violations = new TreeSet<>();
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
                if (!methodName.equals("clearOnDisconnect")) {
                    return super.visitMethodInvocation(invocation, unused);
                }
                if (invocation.getMethodSelect() instanceof MemberSelectTree selection) {
                    String owner = selection.getExpression().toString();
                    if (isManagedStoreOwner(
                        owner,
                        storeFqcns,
                        visibleStoreTypeNames,
                        managedStoreTypeNames,
                        sourceIsManagedStore
                    )) {
                        violations.add("invoke:" + invocation.getMethodSelect());
                    }
                } else if (invocation.getMethodSelect() instanceof IdentifierTree
                    && !staticallyImportedCleaners.isEmpty()) {
                    violations.add("static-import:" + staticallyImportedCleaners);
                }
                return super.visitMethodInvocation(invocation, unused);
            }

            @Override
            public Void visitMemberReference(MemberReferenceTree reference, Void unused) {
                if (!reference.getName().contentEquals("clearOnDisconnect")
                    || !isManagedStoreOwner(
                        reference.getQualifierExpression().toString(),
                        storeFqcns,
                        visibleStoreTypeNames,
                        managedStoreTypeNames,
                        sourceIsManagedStore
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
        if (!(parent instanceof MethodInvocationTree invocation)
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

    private static boolean isManagedStoreOwner(
        String owner,
        Set<String> storeFqcns,
        Set<String> visibleStoreTypeNames,
        Set<String> managedStoreTypeNames,
        boolean sourceIsManagedStore
    ) {
        if (storeFqcns.contains(owner) || visibleStoreTypeNames.contains(owner)) {
            return true;
        }
        int separator = owner.lastIndexOf('.');
        String terminalOwner = separator >= 0 ? owner.substring(separator + 1) : owner;
        return !sourceIsManagedStore && managedStoreTypeNames.contains(terminalOwner);
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
