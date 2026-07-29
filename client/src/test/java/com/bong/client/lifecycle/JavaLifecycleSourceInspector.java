package com.bong.client.lifecycle;

import com.sun.source.tree.ClassTree;
import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.IdentifierTree;
import com.sun.source.tree.ImportTree;
import com.sun.source.tree.MemberReferenceTree;
import com.sun.source.tree.MemberSelectTree;
import com.sun.source.tree.MethodInvocationTree;
import com.sun.source.tree.MethodTree;
import com.sun.source.tree.Tree;
import com.sun.source.util.JavacTask;
import com.sun.source.util.TreeScanner;

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
            .anyMatch(method -> method.getName().contentEquals(entryMethod));
        if (!declaresEntry) {
            throw new AssertionError("必须能定位 production cleaner：" + storeIdentity + "." + entryMethod);
        }
    }

    static void assertNoTestResetCalls(String source, String sourceIdentity) {
        CompilationUnitTree unit = parse(simpleName(sourceIdentity), source);
        new TreeScanner<Void, Void>() {
            @Override
            public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                rejectTestResetCall(
                    invokedMethodName(invocation.getMethodSelect()),
                    sourceIdentity
                );
                return super.visitMethodInvocation(invocation, unused);
            }

            @Override
            public Void visitMemberReference(MemberReferenceTree reference, Void unused) {
                rejectTestResetCall(reference.getName().toString(), sourceIdentity);
                return super.visitMemberReference(reference, unused);
            }
        }.scan(unit, null);
    }

    public static Set<String> declaredMethodNames(String source, String sourceIdentity) {
        CompilationUnitTree unit = parse(simpleName(sourceIdentity), source);
        Set<String> names = new TreeSet<>();
        new TreeScanner<Void, Void>() {
            @Override
            public Void visitMethod(MethodTree method, Void unused) {
                String name = method.getName().toString();
                if (!name.equals("<init>")) {
                    names.add(name);
                }
                return super.visitMethod(method, unused);
            }
        }.scan(unit, null);
        return Set.copyOf(names);
    }

    public static void assertMethodContainsNoForbiddenTokens(
        String source,
        String className,
        String methodName,
        Set<String> forbiddenFqcns,
        Set<String> forbiddenMemberNames,
        Set<String> allowedTokens
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

        Set<String> forbiddenTypeNames = new TreeSet<>();
        for (String fqcn : forbiddenFqcns) {
            forbiddenTypeNames.add(simpleName(fqcn));
        }

        Set<String> rejectedTokens = new TreeSet<>();
        TreeScanner<Void, Void> bodyScanner = new TreeScanner<>() {
            private void reject(String token) {
                if ((forbiddenTypeNames.contains(token) || forbiddenMemberNames.contains(token))
                    && !allowedTokens.contains(token)) {
                    rejectedTokens.add(token);
                }
            }

            @Override
            public Void visitIdentifier(IdentifierTree identifier, Void unused) {
                reject(identifier.getName().toString());
                return super.visitIdentifier(identifier, unused);
            }

            @Override
            public Void visitMemberSelect(MemberSelectTree selection, Void unused) {
                reject(selection.getIdentifier().toString());
                return super.visitMemberSelect(selection, unused);
            }
        };

        for (ImportTree importTree : unit.getImports()) {
            String imported = importTree.getQualifiedIdentifier().toString();
            for (String fqcn : forbiddenFqcns) {
                if (imported.equals(fqcn)
                    || imported.equals(fqcn + ".*")
                    || imported.startsWith(fqcn + ".")) {
                    rejectedTokens.add(imported);
                }
            }
            if (importTree.isStatic()) {
                bodyScanner.scan(importTree.getQualifiedIdentifier(), null);
            }
        }
        bodyScanner.scan(methods.get(0).getBody(), null);

        if (!rejectedTokens.isEmpty()) {
            throw new AssertionError(
                className + "." + methodName
                    + " 只能通过 SessionScopedStoreRegistry.clearAllOnDisconnect 清理 Store；禁用 token="
                    + rejectedTokens
            );
        }
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

    private static void rejectTestResetCall(String methodName, String sourceIdentity) {
        if (TEST_RESET_METHODS.contains(methodName)) {
            throw new AssertionError(
                sourceIdentity + " 的 production source 不得调用或引用 test reset " + methodName
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
