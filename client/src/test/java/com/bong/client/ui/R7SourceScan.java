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
import java.util.function.Predicate;
import java.util.Locale;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.TreeSet;
import java.util.Set;

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

    /**
     * 随资源包发布的资产根。
     *
     * <p>这些字节由资源包 sha1 闸门逐字节钉住（{@code client/resourcepack/manifest.json}
     * 与 {@code server/src/network/resourcepack.rs} 的 sha1 必须等于实际构建出的包），
     * 服务端还会把该 sha1 通过 vanilla 协议下发给客户端校验。在这里再钉一遍买不到任何
     * 额外保证，只会让每个美术资产 PR 都要重新冻结一次摘要。
     */
    /**
     * 读 {@code production-source-baseline.tsv} 里某个 scope 的冻结摘要。
     *
     * <p>基线放 fixture 而不是源码字面量：重新冻结时 PR diff 里就是「某个 scope 的
     * 摘要变了」一行，而不是埋在一段二十行的 Java 注释中间。
     */
    static String baselineDigest(String scope) throws IOException {
        try (var stream = R7SourceScan.class.getResourceAsStream("/bong/ui/production-source-baseline.tsv")) {
            if (stream == null) {
                throw new AssertionError("missing production source baseline fixture");
            }
            String text = new String(stream.readAllBytes(), StandardCharsets.UTF_8);
            for (String line : text.lines().toList()) {
                if (!isFixtureDataLine(line)) {
                    continue;
                }
                String[] fields = line.split("\\t", -1);
                if (fields.length == 3 && fields[0].equals(scope)) {
                    if (!fields[1].equals("SHA-256")) {
                        throw new AssertionError("scope " + scope + " must use the pinned digest algorithm");
                    }
                    return fields[2];
                }
            }
        }
        throw new AssertionError("production source baseline has no row for scope " + scope);
    }

    static Path shippedAssetRoot() {
        return productionInputRoot().resolve("resources").resolve("assets");
    }

    /** 生成一个「排除 excludedRoot 子树」的谓词。做成参数化是为了能对临时树做差分注入。 */
    static Predicate<Path> excluding(Path excludedRoot) {
        Path normalized = excludedRoot.toAbsolutePath().normalize();
        return path -> !path.toAbsolutePath().normalize().startsWith(normalized);
    }

    private static final Pattern PACK_PREFIX_BLOCK =
        Pattern.compile("INCLUDE_PREFIXES=\\((.*?)\\)", Pattern.DOTALL);
    private static final Pattern PACK_QUOTED = Pattern.compile("\"([^\"]+)\"");
    private static final Pattern PACK_EXTENSION_CASE =
        Pattern.compile("^\\s*(\\*\\.[a-z0-9|*.]+)\\)\\s*;;\\s*$", Pattern.MULTILINE);

    static Path resourcePackScript() {
        return repositoryRoot().resolve("scripts").resolve("build-resourcepack.sh");
    }

    /**
     * 打包器实际会收进资源包的路径前缀，**从 build-resourcepack.sh 解析**而不是在这里手抄。
     *
     * <p>手抄一份必然漂：打包器加一个 prefix，这边不知道，那批文件就同时从两道闸门底下漏掉。
     * 解析不出来直接炸，不静默退化成空集合（空集合会让整个 assets 目录重新落回本基线，
     * 撞红总比漏掉强，但更该做的是立刻告诉人「脚本格式变了」）。
     */
    static List<String> resourcePackIncludedPrefixes() {
        String script = read(resourcePackScript());
        Matcher block = PACK_PREFIX_BLOCK.matcher(script);
        if (!block.find()) {
            throw new AssertionError(
                "无法从 " + resourcePackScript() + " 解析 INCLUDE_PREFIXES —— 打包脚本格式变了？"
                + "本基线靠它划分「谁归资源包 sha1 管、谁归本基线管」，解析不出来必须停下。");
        }
        List<String> prefixes = new ArrayList<>();
        Matcher quoted = PACK_QUOTED.matcher(block.group(1));
        while (quoted.find()) {
            prefixes.add(quoted.group(1).toLowerCase(Locale.ROOT));
        }
        if (prefixes.isEmpty()) {
            throw new AssertionError("INCLUDE_PREFIXES 解析结果为空：" + resourcePackScript());
        }
        return List.copyOf(prefixes);
    }

    /** 打包器接受的扩展名，同样从脚本的 case 分支解析。 */
    static Set<String> resourcePackIncludedExtensions() {
        String script = read(resourcePackScript());
        Matcher matcher = PACK_EXTENSION_CASE.matcher(script);
        if (!matcher.find()) {
            throw new AssertionError(
                "无法从 " + resourcePackScript() + " 解析打包扩展名白名单 —— 打包脚本格式变了？");
        }
        Set<String> extensions = new TreeSet<>();
        for (String token : matcher.group(1).split("\\|")) {
            String trimmed = token.trim();
            if (trimmed.startsWith("*.")) {
                extensions.add(trimmed.substring(1).toLowerCase(Locale.ROOT));
            }
        }
        if (extensions.isEmpty()) {
            throw new AssertionError("打包扩展名白名单解析结果为空：" + resourcePackScript());
        }
        return Set.copyOf(extensions);
    }

    /**
     * 该文件是否会被 build-resourcepack.sh 打进资源包。
     *
     * <p>**只有这些字节才由资源包 sha1 闸门钉住**。assets 目录下另有一大批文件（当前 580 个，
     * 例如 {@code bong-client/textures/gui/items/*}）不在 INCLUDE_PREFIXES 里，进不了资源包，
     * 但它们随 mod jar 发布——照样是随包字节，必须留在本基线里。
     */
    static boolean isResourcePackAsset(Path path) {
        return isResourcePackAsset(path, shippedAssetRoot(),
            resourcePackIncludedExtensions(), resourcePackIncludedPrefixes());
    }

    /** 可参数化版本，供对临时树做差分注入。 */
    static boolean isResourcePackAsset(Path path, Path assetRoot,
                                       Set<String> extensions, List<String> prefixes) {
        Path root = assetRoot.toAbsolutePath().normalize();
        Path normalized = path.toAbsolutePath().normalize();
        if (!normalized.startsWith(root)) {
            return false;
        }
        String relative = root.relativize(normalized).toString().replace('\\', '/')
            .toLowerCase(Locale.ROOT);
        if (extensions.stream().noneMatch(relative::endsWith)) {
            return false;
        }
        return prefixes.stream()
            .anyMatch(prefix -> relative.equals(prefix) || relative.startsWith(prefix + "/"));
    }

    /** 该文件是否**不**由资源包 sha1 闸门钉住，因而必须留在 R7 冻结基线里。 */
    static boolean isNotShippedAsset(Path path) {
        return !isResourcePackAsset(path);
    }

    static long treeFileCount(Path root, Predicate<Path> include) throws IOException {
        try (var files = Files.walk(root)) {
            return files.filter(Files::isRegularFile).filter(include).count();
        }
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

    static String sourceTreeDigest(Path root) throws IOException {
        return sourceTreeDigest(root, path -> true);
    }

    static String sourceTreeDigest(Path root, Predicate<Path> include) throws IOException {
        MessageDigest digest;
        try {
            digest = MessageDigest.getInstance("SHA-256");
        } catch (NoSuchAlgorithmException exception) {
            throw new AssertionError("SHA-256 unavailable", exception);
        }
        try (var files = Files.walk(root)) {
            for (Path path : files.filter(Files::isRegularFile).filter(include).sorted().toList()) {
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
