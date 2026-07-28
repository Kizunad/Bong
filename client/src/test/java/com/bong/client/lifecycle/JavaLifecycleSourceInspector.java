package com.bong.client.lifecycle;

import com.sun.source.tree.ClassTree;
import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.IdentifierTree;
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
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

final class JavaLifecycleSourceInspector {
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

    static void assertCleanerDoesNotReachTestReset(
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
        Map<String, List<MethodTree>> methodsByName = new HashMap<>();
        for (Tree member : storeType.getMembers()) {
            if (member instanceof MethodTree method) {
                methodsByName.computeIfAbsent(method.getName().toString(), ignored -> new ArrayList<>())
                    .add(method);
            }
        }

        List<MethodTree> entries = methodsByName.getOrDefault(entryMethod, List.of());
        if (entries.isEmpty()) {
            throw new AssertionError("必须能定位 production cleaner：" + storeIdentity + "." + entryMethod);
        }

        ArrayDeque<MethodTree> pending = new ArrayDeque<>(entries);
        Set<MethodTree> visited = new HashSet<>();
        Set<String> reachableNames = new HashSet<>();
        while (!pending.isEmpty()) {
            MethodTree method = pending.removeFirst();
            if (!visited.add(method)) {
                continue;
            }
            String methodName = method.getName().toString();
            reachableNames.add(methodName);
            rejectTestReset(methodName, storeIdentity, reachableNames);
            if (method.getBody() == null) {
                continue;
            }
            new TreeScanner<Void, Void>() {
                @Override
                public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                    String callee = invokedMethodName(invocation.getMethodSelect());
                    rejectTestReset(callee, storeIdentity, reachableNames);
                    pending.addAll(methodsByName.getOrDefault(callee, List.of()));
                    return super.visitMethodInvocation(invocation, unused);
                }

                @Override
                public Void visitMemberReference(MemberReferenceTree reference, Void unused) {
                    String callee = reference.getName().toString();
                    rejectTestReset(callee, storeIdentity, reachableNames);
                    pending.addAll(methodsByName.getOrDefault(callee, List.of()));
                    return super.visitMemberReference(reference, unused);
                }
            }.scan(method.getBody(), null);
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

    private static void rejectTestReset(
        String methodName,
        String storeIdentity,
        Set<String> reachableNames
    ) {
        if (TEST_RESET_METHODS.contains(methodName)) {
            throw new AssertionError(
                storeIdentity + " 的 production cleaner 委托链不得调用 " + methodName
                    + "；可达方法=" + reachableNames
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
