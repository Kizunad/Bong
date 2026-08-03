package com.bong.client.ui;

import com.sun.source.tree.ClassTree;
import com.sun.source.tree.CompilationUnitTree;
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
import java.util.HexFormat;
import java.util.List;

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

    static Path productionInputRoot() {
        return productionRoot().getParent().getParent().getParent().getParent();
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

    static String sourceTreeDigest(Path root) throws IOException {
        MessageDigest digest;
        try {
            digest = MessageDigest.getInstance("SHA-256");
        } catch (NoSuchAlgorithmException exception) {
            throw new AssertionError("SHA-256 unavailable", exception);
        }
        try (var files = Files.walk(root)) {
            for (Path path : files.filter(Files::isRegularFile)
                .sorted()
                .toList()) {
                digest.update(root.relativize(path).toString().replace('\\', '/').getBytes(StandardCharsets.UTF_8));
                digest.update((byte) 0);
                digest.update(MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(path)));
                digest.update((byte) '\n');
            }
        } catch (NoSuchAlgorithmException exception) {
            throw new AssertionError("SHA-256 unavailable", exception);
        }
        return HexFormat.of().formatHex(digest.digest());
    }

    static List<TokenOccurrence> tokenOccurrences(Path root, String token) throws IOException {
        List<TokenOccurrence> occurrences = new ArrayList<>();
        try (var files = Files.walk(root)) {
            for (Path path : files.filter(Files::isRegularFile)
                .filter(candidate -> candidate.getFileName().toString().endsWith(".java"))
                .sorted()
                .toList()) {
                occurrences.addAll(tokenOccurrences(root, path, token));
            }
        }
        return occurrences;
    }

    static List<StructuralTokenOccurrence> structuralTokenOccurrences(
        Path root,
        String token
    ) throws IOException {
        List<StructuralTokenOccurrence> occurrences = new ArrayList<>();
        try (var files = Files.walk(root)) {
            for (Path path : files.filter(Files::isRegularFile)
                .filter(candidate -> candidate.getFileName().toString().endsWith(".java"))
                .sorted()
                .toList()) {
                List<TokenOccurrence> lexical = tokenOccurrences(root, path, token);
                if (lexical.stream().noneMatch(TokenOccurrence::code)) {
                    continue;
                }
                String source = read(path);
                occurrences.addAll(structuralTokenOccurrences(path, source, lexical));
            }
        }
        return occurrences;
    }

    static String codeOnly(String source) {
        StringBuilder result = new StringBuilder(source.length());
        LexState state = LexState.CODE;
        for (int index = 0; index < source.length(); index++) {
            char current = source.charAt(index);
            char next = index + 1 < source.length() ? source.charAt(index + 1) : '\0';
            switch (state) {
                case CODE -> {
                    if (current == '/' && next == '/') {
                        result.append("  ");
                        index++;
                        state = LexState.LINE_COMMENT;
                    } else if (current == '/' && next == '*') {
                        result.append("  ");
                        index++;
                        state = LexState.BLOCK_COMMENT;
                    } else if (current == '"') {
                        result.append(' ');
                        state = LexState.STRING;
                    } else if (current == '\'') {
                        result.append(' ');
                        state = LexState.CHARACTER;
                    } else {
                        result.append(current);
                    }
                }
                case LINE_COMMENT -> {
                    result.append(current == '\n' ? '\n' : ' ');
                    if (current == '\n') {
                        state = LexState.CODE;
                    }
                }
                case BLOCK_COMMENT -> {
                    if (current == '*' && next == '/') {
                        result.append("  ");
                        index++;
                        state = LexState.CODE;
                    } else {
                        result.append(current == '\n' ? '\n' : ' ');
                    }
                }
                case STRING, CHARACTER -> {
                    result.append(current == '\n' ? '\n' : ' ');
                    if (current == '\\' && next != '\0') {
                        result.append(next == '\n' ? '\n' : ' ');
                        index++;
                    } else if ((state == LexState.STRING && current == '"')
                        || (state == LexState.CHARACTER && current == '\'')) {
                        state = LexState.CODE;
                    }
                }
            }
        }
        return result.toString();
    }

    private static List<TokenOccurrence> tokenOccurrences(Path root, Path path, String token) {
        String source = read(path);
        String[] lines = source.split("\\R", -1);
        List<TokenOccurrence> occurrences = new ArrayList<>();
        LexState state = LexState.CODE;
        int line = 1;
        int ordinal = 0;

        for (int index = 0; index < source.length();) {
            if (source.startsWith(token, index)) {
                ordinal++;
                occurrences.add(new TokenOccurrence(
                    root.relativize(path).toString().replace('\\', '/'),
                    ordinal,
                    line,
                    index,
                    state == LexState.CODE,
                    lines[line - 1].strip()
                ));
                index += token.length();
                continue;
            }

            char current = source.charAt(index);
            char next = index + 1 < source.length() ? source.charAt(index + 1) : '\0';
            switch (state) {
                case CODE -> {
                    if (current == '/' && next == '/') {
                        state = LexState.LINE_COMMENT;
                        index += 2;
                        continue;
                    }
                    if (current == '/' && next == '*') {
                        state = LexState.BLOCK_COMMENT;
                        index += 2;
                        continue;
                    }
                    if (current == '"') {
                        state = LexState.STRING;
                    } else if (current == '\'') {
                        state = LexState.CHARACTER;
                    }
                }
                case LINE_COMMENT -> {
                    if (current == '\n') {
                        state = LexState.CODE;
                    }
                }
                case BLOCK_COMMENT -> {
                    if (current == '*' && next == '/') {
                        state = LexState.CODE;
                        index += 2;
                        continue;
                    }
                }
                case STRING, CHARACTER -> {
                    if (current == '\\' && next != '\0') {
                        if (next == '\n') {
                            line++;
                        }
                        index += 2;
                        continue;
                    }
                    if ((state == LexState.STRING && current == '"')
                        || (state == LexState.CHARACTER && current == '\'')) {
                        state = LexState.CODE;
                    }
                }
            }
            if (current == '\n') {
                line++;
            }
            index++;
        }
        return occurrences;
    }

    private static List<StructuralTokenOccurrence> structuralTokenOccurrences(
        Path path,
        String source,
        List<TokenOccurrence> occurrences
    ) {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        if (compiler == null) {
            throw new AssertionError("R7 fill inventory requires a full Java 17 JDK, not a JRE");
        }
        DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
        try (StandardJavaFileManager fileManager = compiler.getStandardFileManager(diagnostics, null, null)) {
            Iterable<? extends JavaFileObject> sources = fileManager.getJavaFileObjects(path.toFile());
            JavacTask task = (JavacTask) compiler.getTask(
                null,
                fileManager,
                diagnostics,
                List.of("-proc:none"),
                null,
                sources
            );
            Trees trees = Trees.instance(task);
            CompilationUnitTree unit = task.parse().iterator().next();
            List<EnclosingTree> enclosingTrees = new ArrayList<>();
            new TreePathScanner<Void, Void>() {
                @Override
                public Void visitClass(ClassTree classTree, Void unused) {
                    addTree(classTree, "CLASS:" + classTree.getSimpleName());
                    return super.visitClass(classTree, unused);
                }

                @Override
                public Void visitMethod(MethodTree methodTree, Void unused) {
                    addTree(methodTree, "METHOD:" + methodSignature(methodTree));
                    return super.visitMethod(methodTree, unused);
                }

                private void addTree(com.sun.source.tree.Tree tree, String identity) {
                    long start = trees.getSourcePositions().getStartPosition(unit, tree);
                    long end = trees.getSourcePositions().getEndPosition(unit, tree);
                    if (start >= 0 && end >= start) {
                        enclosingTrees.add(new EnclosingTree(start, end, identity, digest(source.substring(
                            Math.toIntExact(start),
                            Math.toIntExact(end)
                        ))));
                    }
                }
            }.scan(unit, null);
            boolean parseFailed = diagnostics.getDiagnostics().stream()
                .anyMatch(diagnostic -> diagnostic.getKind() == Diagnostic.Kind.ERROR);
            if (parseFailed) {
                throw new AssertionError(
                    "unable to parse fill inventory source " + path + ": " + diagnostics.getDiagnostics());
            }
            enclosingTrees.sort(Comparator.comparingLong(tree -> tree.end() - tree.start()));
            List<StructuralTokenOccurrence> result = new ArrayList<>();
            for (TokenOccurrence occurrence : occurrences) {
                if (!occurrence.code()) {
                    continue;
                }
                int offset = occurrence.offset();
                EnclosingTree method = enclosingTrees.stream()
                    .filter(tree -> tree.identity().startsWith("METHOD:"))
                    .filter(tree -> tree.contains(offset))
                    .findFirst()
                    .orElseThrow(() -> new AssertionError(
                        "unable to locate enclosing method for " + path + "#" + occurrence.ordinal()));
                EnclosingTree clazz = enclosingTrees.stream()
                    .filter(tree -> tree.identity().startsWith("CLASS:"))
                    .filter(tree -> tree.contains(offset))
                    .findFirst()
                    .orElseThrow(() -> new AssertionError(
                        "unable to locate enclosing class for " + path + "#" + occurrence.ordinal()));
                result.add(new StructuralTokenOccurrence(
                    occurrence.stableKey(),
                    clazz.identity().substring("CLASS:".length()),
                    method.identity().substring("METHOD:".length()),
                    method.digest()
                ));
            }
            return result;
        } catch (IOException exception) {
            throw new AssertionError("unable to parse fill inventory source " + path, exception);
        }
    }

    private static String methodSignature(MethodTree methodTree) {
        return methodTree.getName() + "(" + methodTree.getParameters().stream()
            .map(parameter -> parameter.getType().toString().replaceAll("\\s+", ""))
            .reduce((left, right) -> left + "," + right)
            .orElse("") + ")";
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

    private record EnclosingTree(long start, long end, String identity, String digest) {
        boolean contains(long offset) {
            return start <= offset && offset < end;
        }
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

    private enum LexState {
        CODE,
        LINE_COMMENT,
        BLOCK_COMMENT,
        STRING,
        CHARACTER
    }
}
