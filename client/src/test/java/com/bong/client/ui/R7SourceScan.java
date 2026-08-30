package com.bong.client.ui;

import com.sun.source.tree.ClassTree;
import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.MemberSelectTree;
import com.sun.source.tree.MethodInvocationTree;
import com.sun.source.tree.MethodTree;
import com.sun.source.util.JavacTask;
import com.sun.source.util.TreePathScanner;
import com.sun.source.util.Trees;

import javax.tools.Diagnostic;
import javax.tools.DiagnosticCollector;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HexFormat;
import java.util.List;
import java.util.Map;

final class R7SourceScan {
    private R7SourceScan() {
    }

    static Path productionRoot() {
        Path workingDirectory = Path.of("").toAbsolutePath().normalize();
        Path clientRoot = Files.isDirectory(workingDirectory.resolve("src"))
            ? workingDirectory
            : workingDirectory.resolve("client");
        return clientRoot.resolve("src/main/java/com/bong/client");
    }

    static Path repositoryRoot() {
        return productionRoot()
            .getParent()
            .getParent()
            .getParent()
            .getParent()
            .getParent()
            .getParent()
            .getParent();
    }

    static String read(Path path) {
        try {
            return Files.readString(path);
        } catch (IOException exception) {
            throw new AssertionError("无法读取 R7 contract source：" + path.toAbsolutePath(), exception);
        }
    }

    static boolean isFixtureDataLine(String line) {
        return !line.isBlank()
            && !line.stripLeading().startsWith("#")
            && !line.matches("\\s*\\d+\\t\\s*#.*");
    }

    static List<ParsedUnit> parseJava(Path root) throws IOException {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        if (compiler == null) {
            throw new AssertionError("R7 production contracts require a full Java 17 JDK, not a JRE");
        }
        List<Path> paths;
        try (var files = Files.walk(root)) {
            paths = files.filter(Files::isRegularFile)
                .filter(path -> path.getFileName().toString().endsWith(".java"))
                .sorted()
                .toList();
        }
        DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
        try (StandardJavaFileManager fileManager = compiler.getStandardFileManager(diagnostics, null, null)) {
            JavacTask task = (JavacTask) compiler.getTask(
                null,
                fileManager,
                diagnostics,
                List.of("-proc:none", "-classpath", System.getProperty("java.class.path")),
                null,
                fileManager.getJavaFileObjectsFromPaths(paths)
            );
            Trees trees = Trees.instance(task);
            List<CompilationUnitTree> parsedUnits = new ArrayList<>();
            task.parse().forEach(parsedUnits::add);
            boolean parseFailed = diagnostics.getDiagnostics().stream()
                .anyMatch(diagnostic -> diagnostic.getKind() == Diagnostic.Kind.ERROR);
            if (parseFailed) {
                throw new AssertionError("unable to parse R7 production sources: " + diagnostics.getDiagnostics());
            }
            task.analyze();
            List<ParsedUnit> units = new ArrayList<>();
            for (CompilationUnitTree unit : parsedUnits) {
                Path path = Path.of(unit.getSourceFile().toUri());
                units.add(new ParsedUnit(path, read(path), unit, trees));
            }
            units.sort(Comparator.comparing(unit -> unit.path().toString()));
            return units;
        }
    }

    static List<TokenOccurrence> tokenOccurrences(Path root, String token) throws IOException {
        List<TokenOccurrence> result = new ArrayList<>();
        for (ParsedUnit parsed : parseJava(root)) {
            Map<Integer, StructuralTokenOccurrence> executable = executableTokenContexts(root, parsed, token);
            String[] lines = parsed.source().split("\\R", -1);
            int ordinal = 0;
            for (int offset = parsed.source().indexOf(token); offset >= 0;
                 offset = parsed.source().indexOf(token, offset + token.length())) {
                ordinal++;
                int line = Math.toIntExact(parsed.unit().getLineMap().getLineNumber(offset));
                result.add(new TokenOccurrence(
                    relative(root, parsed.path()),
                    ordinal,
                    line,
                    offset,
                    executable.containsKey(offset),
                    lines[Math.min(line - 1, lines.length - 1)].strip()
                ));
            }
        }
        return result;
    }

    static List<StructuralTokenOccurrence> structuralTokenOccurrences(Path root, String token) throws IOException {
        List<StructuralTokenOccurrence> result = new ArrayList<>();
        for (ParsedUnit parsed : parseJava(root)) {
            Map<Integer, StructuralTokenOccurrence> executable = executableTokenContexts(root, parsed, token);
            int ordinal = 0;
            for (int offset = parsed.source().indexOf(token); offset >= 0;
                 offset = parsed.source().indexOf(token, offset + token.length())) {
                ordinal++;
                StructuralTokenOccurrence context = executable.get(offset);
                if (context != null) {
                    result.add(new StructuralTokenOccurrence(
                        relative(root, parsed.path()) + "#" + ordinal,
                        context.enclosingClass(),
                        context.enclosingMethod(),
                        context.enclosingMethodDigest()
                    ));
                }
            }
        }
        return result;
    }

    static List<String> zeroArgumentInvocationSites(Path root, String methodName) throws IOException {
        List<String> result = new ArrayList<>();
        for (ParsedUnit parsed : parseJava(root)) {
            new TreePathScanner<Void, Void>() {
                @Override
                public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                    if (invocation.getArguments().isEmpty()
                        && invocation.getMethodSelect() instanceof MemberSelectTree select
                        && select.getIdentifier().contentEquals(methodName)) {
                        long offset = parsed.trees().getSourcePositions()
                            .getStartPosition(parsed.unit(), invocation);
                        long line = parsed.unit().getLineMap().getLineNumber(offset);
                        result.add(relative(root, parsed.path()) + ":" + line);
                    }
                    return super.visitMethodInvocation(invocation, unused);
                }
            }.scan(parsed.unit(), null);
        }
        result.sort(String::compareTo);
        return result;
    }

    private static Map<Integer, StructuralTokenOccurrence> executableTokenContexts(
        Path root,
        ParsedUnit parsed,
        String token
    ) {
        Map<Integer, StructuralTokenOccurrence> result = new HashMap<>();
        new TreePathScanner<Void, Void>() {
            private String enclosingClass;
            private MethodTree enclosingMethod;

            @Override
            public Void visitClass(ClassTree tree, Void unused) {
                String previous = enclosingClass;
                enclosingClass = tree.getSimpleName().toString();
                try {
                    return super.visitClass(tree, unused);
                } finally {
                    enclosingClass = previous;
                }
            }

            @Override
            public Void visitMethod(MethodTree tree, Void unused) {
                MethodTree previous = enclosingMethod;
                enclosingMethod = tree;
                try {
                    return super.visitMethod(tree, unused);
                } finally {
                    enclosingMethod = previous;
                }
            }

            @Override
            public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                long start = parsed.trees().getSourcePositions().getStartPosition(parsed.unit(), invocation);
                long end = parsed.trees().getSourcePositions().getEndPosition(parsed.unit(), invocation);
                if (start >= 0 && end >= start
                    && parsed.source().substring(Math.toIntExact(start), Math.toIntExact(end)).equals(token)) {
                    if (enclosingClass == null || enclosingMethod == null) {
                        throw new AssertionError("executable token lacks enclosing declaration in " + parsed.path());
                    }
                    long methodStart = parsed.trees().getSourcePositions()
                        .getStartPosition(parsed.unit(), enclosingMethod);
                    long methodEnd = parsed.trees().getSourcePositions()
                        .getEndPosition(parsed.unit(), enclosingMethod);
                    result.put(Math.toIntExact(start), new StructuralTokenOccurrence(
                        relative(root, parsed.path()),
                        enclosingClass,
                        methodSignature(enclosingMethod),
                        digest(parsed.source().substring(Math.toIntExact(methodStart), Math.toIntExact(methodEnd)))
                    ));
                }
                return super.visitMethodInvocation(invocation, unused);
            }
        }.scan(parsed.unit(), null);
        return result;
    }

    private static String methodSignature(MethodTree method) {
        return method.getName() + "(" + method.getParameters().stream()
            .map(parameter -> parameter.getType().toString().replaceAll("\\s+", ""))
            .reduce((left, right) -> left + "," + right)
            .orElse("") + ")";
    }

    private static String relative(Path root, Path path) {
        return root.relativize(path).toString().replace('\\', '/');
    }

    private static String digest(String value) {
        try {
            return HexFormat.of().formatHex(
                MessageDigest.getInstance("SHA-256").digest(value.getBytes(StandardCharsets.UTF_8))
            );
        } catch (NoSuchAlgorithmException exception) {
            throw new AssertionError("SHA-256 unavailable", exception);
        }
    }

    record ParsedUnit(Path path, String source, CompilationUnitTree unit, Trees trees) {
    }

    record TokenOccurrence(String path, int ordinal, int line, int offset, boolean code, String sourceLine) {
        String stableKey() {
            return path + "#" + ordinal;
        }
    }

    record StructuralTokenOccurrence(
        String stableKey,
        String enclosingClass,
        String enclosingMethod,
        String enclosingMethodDigest
    ) {
    }
}
