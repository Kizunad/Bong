package com.bong.client.ui;

import com.sun.source.tree.ClassTree;
import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.ExpressionTree;
import com.sun.source.tree.MemberSelectTree;
import com.sun.source.tree.MethodInvocationTree;
import com.sun.source.tree.MethodTree;
import com.sun.source.tree.ParenthesizedTree;
import com.sun.source.tree.TypeCastTree;
import com.sun.source.util.JavacTask;
import com.sun.source.util.TreePathScanner;
import io.wispforest.owo.ui.core.Sizing;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import javax.tools.DiagnosticCollector;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.SimpleJavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;
import java.io.IOException;
import java.net.URISyntaxException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class R7InventoryContractTest {
    private static final Path PRODUCTION_ROOT = R7SourceScan.productionRoot();
    private static final Path PRODUCTION_INPUT_ROOT = R7SourceScan.productionInputRoot();

    @Test
    void screenInventoryPinsEveryDirectProductionScreenAndSuffixException() throws IOException {
        List<ScreenInventoryRow> expectedRows = readScreenInventory();
        List<ScreenInventoryRow> actualRows = discoverDirectScreensAndSuffixHelpers();

        assertEquals(expectedRows, actualRows,
            "R7 Screen inventory drifted: every direct Screen and every *Screen.java false positive must be classified");
        assertEquals(30, expectedRows.size(), "fixture should contain 29 suffix files plus one non-suffix Screen");
        assertEquals(15, count(expectedRows, "BASE_OWO"), "direct owo migration set changed");
        assertEquals(14, count(expectedRows, "VANILLA_SCREEN"), "direct vanilla Screen set changed");
        assertEquals(1, count(expectedRows, "NON_SCREEN_HELPER"), "Screen.java false-positive set changed");
        assertEquals(15, expectedRows.stream().filter(ScreenInventoryRow::eligible).count(),
            "P1 base migration is limited to direct owo Screens");
        assertTrue(expectedRows.stream().anyMatch(row -> row.path().equals(
            "cultivation/voidaction/LegacyAssignPanel.java")),
            "suffix-only discovery must not lose a real Screen named LegacyAssignPanel");
        assertTrue(expectedRows.stream().anyMatch(row -> row.path().equals(
            "cultivation/TechniqueScrollReadScreen.java") && row.kind().equals("NON_SCREEN_HELPER")),
            "suffix-only discovery must not count TechniqueScrollReadScreen as a Screen");
    }

    @Test
    void fill100InventoryPinsEveryTokenWithLexicalState() throws IOException {
        List<FillInventoryRow> expectedRows = readFillInventory();
        List<R7SourceScan.TokenOccurrence> actual = R7SourceScan.tokenOccurrences(PRODUCTION_ROOT, "Sizing.fill(100)");

        assertEquals(92, actual.size(), "raw Sizing.fill(100) inventory must include code and comments");
        assertEquals(
            expectedRows.stream().map(FillInventoryRow::stableKey).toList(),
            actual.stream().map(R7SourceScan.TokenOccurrence::stableKey).toList(),
            "Sizing.fill(100) path-local occurrence inventory drifted; classify each added or removed token explicitly"
        );
        assertEquals(
            expectedRows.stream().map(FillInventoryRow::code).toList(),
            actual.stream().map(R7SourceScan.TokenOccurrence::code).toList(),
            "a token moved between executable code and comment/string context"
        );
        assertEquals(
            expectedRows.stream().map(FillInventoryRow::freezeLine).toList(),
            actual.stream().map(R7SourceScan.TokenOccurrence::line).toList(),
            "freeze-line diagnostics drifted; re-audit the affected layout context"
        );
        assertEquals(
            expectedRows.stream().map(FillInventoryRow::source).toList(),
            actual.stream().map(R7SourceScan.TokenOccurrence::sourceLine).toList(),
            "source context drifted; retain or explicitly reclassify each fill(100) occurrence"
        );
        assertEquals(
            expectedFillClassifications(),
            expectedRows.stream()
                .map(row -> row.stableKey() + "\t" + row.verdict() + "\t" + row.riskKind())
                .toList(),
            "every exact fill(100) token verdict and risk kind must be explicitly re-decided"
        );

        assertEquals(
            readFillStructuralContext(),
            R7SourceScan.structuralTokenOccurrences(PRODUCTION_ROOT, "Sizing.fill(100)"),
            "code fill(100) context drifted: re-audit the enclosing class/method and every sibling or parent edit"
        );
        assertEquals(87, readFillStructuralContext().size(),
            "all 87 executable fill(100) tokens must carry a structural method snapshot");

        Map<String, Long> verdicts = histogram(expectedRows.stream().map(FillInventoryRow::verdict).toList());
        assertEquals(Map.of("COMMENT", 5L, "LEGAL", 82L, "RISK", 5L), verdicts,
            "P0 context-aware classification is frozen at 82 accepted, 5 risks, and 5 comments");
        Map<String, Long> risks = histogram(expectedRows.stream()
            .map(FillInventoryRow::riskKind)
            .filter(kind -> !kind.equals("NONE"))
            .toList());
        assertEquals(Map.of(
            "EVICTS_LATER_SIBLING", 4L,
            "TERMINAL_INTENTIONAL", 3L,
            "TERMINAL_ORDER_DEPENDENT", 1L
        ), risks, "main-axis overflow classification drifted");
        assertEquals(20, actual.stream().map(R7SourceScan.TokenOccurrence::path).distinct().count(),
            "fill(100) token file count changed");
    }

    @Test
    void fillStructuralContextChangesWhenAnAdjacentSiblingChanges(@TempDir Path directory) throws IOException {
        Path source = directory.resolve("Probe.java");
        Files.writeString(source, """
            class Probe {
                void build() {
                    root.child(Containers.horizontalFlow(Sizing.fill(100), Sizing.content()));
                }
            }
            """);
        String before = R7SourceScan.structuralTokenOccurrences(directory, "Sizing.fill(100)")
            .get(0)
            .enclosingMethodDigest();

        Files.writeString(source, """
            class Probe {
                void build() {
                    root.child(Containers.horizontalFlow(Sizing.fill(100), Sizing.content()));
                    root.child(nextSibling);
                }
            }
            """);
        String after = R7SourceScan.structuralTokenOccurrences(directory, "Sizing.fill(100)")
            .get(0)
            .enclosingMethodDigest();

        assertFalse(before.equals(after),
            "an unchanged fill token must still detect sibling-order changes in its enclosing layout method");
    }

    @Test
    void owoFillInflatesAgainstTheWholeAvailableSpace() {
        assertEquals(0, Sizing.fill(100).inflate(0, ignored -> 43));
        assertEquals(73, Sizing.fill(100).inflate(73, ignored -> 43));
        assertEquals(200, Sizing.fill(100).inflate(200, ignored -> 43));
        assertEquals(50, Sizing.fill(25).inflate(200, ignored -> 43));
        assertEquals(16, Sizing.content(3).inflate(10_000, ignored -> 10));
    }

    @Test
    void tokenOccurrencesClassifiesStringCharacterAndEscapedLiteralStates(@TempDir Path directory)
        throws IOException {
        Path source = directory.resolve("Probe.java");
        Files.writeString(source, """
            class Probe {
                void build() {
                    use(Sizing.fill(100));
                    String plain = "Sizing.fill(100)";
                    String escaped = "\\\"Sizing.fill(100)";
                    char quote = '\\''; // Sizing.fill(100)
                    char marker = '§';
                    char slash = '\\\\'; /* Sizing.fill(100) */
                }
            }
            """);

        List<R7SourceScan.TokenOccurrence> occurrences =
            R7SourceScan.tokenOccurrences(directory, "Sizing.fill(100)");
        assertEquals(5, occurrences.size(), "fixture must cover code, two strings, and two comments");
        assertEquals(List.of(true, false, false, false, false),
            occurrences.stream().map(R7SourceScan.TokenOccurrence::code).toList(),
            "literal and comment token occurrences must never be classified as executable code");
        assertTrue(R7SourceScan.codeOnly(R7SourceScan.read(source)).contains("use(Sizing.fill(100))"),
            "codeOnly must retain the executable occurrence");
        assertEquals(1, java.util.regex.Pattern.compile("Sizing\\.fill\\(100\\)")
            .matcher(R7SourceScan.codeOnly(R7SourceScan.read(source))).results().count(),
            "codeOnly must erase string, character-adjacent, and comment occurrences including escapes");
        assertEquals(List.of(false), R7SourceScan.tokenOccurrences(directory, "§").stream()
            .map(R7SourceScan.TokenOccurrence::code)
            .toList(), "a token inside a character literal must be classified as non-code");
    }

    @Test
    void p0ProductionSourceTreeMatchesFrozenBaseline() throws IOException {
        assertEquals(
            "dbc8d6ad35718cd3d3819e92edc3c124883bcab3e6fe6cd160b112c78de736a4",
            R7SourceScan.sourceTreeDigest(PRODUCTION_INPUT_ROOT),
            "P0 is docs/tests/resources only; every shipped production path and byte must match the frozen baseline"
        );
    }

    @Test
    void p0AddsNoProductionFoundationOrScreenMigration() throws IOException {
        Set<String> forbiddenProductionTypes = Set.of(
            "BongScreenBase.java",
            "DiffListWidget.java",
            "BongKeybindRegistry.java",
            "ClientThreadMarshal.java",
            "ScreenOpenPolicy.java"
        );
        Set<String> discovered = new TreeSet<>();
        try (var files = Files.walk(PRODUCTION_ROOT)) {
            files.filter(Files::isRegularFile)
                .map(path -> path.getFileName().toString())
                .filter(forbiddenProductionTypes::contains)
                .forEach(discovered::add);
        }
        assertTrue(discovered.isEmpty(), "P0 is docs/tests/resources only; production foundation found=" + discovered);

        for (ScreenInventoryRow row : readScreenInventory()) {
            if (!row.kind().equals("BASE_OWO")) {
                continue;
            }
            String code = R7SourceScan.codeOnly(R7SourceScan.read(PRODUCTION_ROOT.resolve(row.path())));
            assertTrue(code.contains("extends BaseOwoScreen<FlowLayout>"),
                "P0 must not migrate production Screen inheritance: " + row.path());
            assertFalse(code.contains("extends BongScreenBase"),
                "P0 must not introduce production behavior: " + row.path());
        }
    }

    private static List<ScreenInventoryRow> discoverDirectScreensAndSuffixHelpers() throws IOException {
        List<ScreenInventoryRow> result = new ArrayList<>();
        for (Path path : productionJavaFiles()) {
            String relative = relative(path);
            String source = R7SourceScan.read(path);
            List<DirectScreenDeclaration> declarations = directScreenDeclarations(path);
            if (!declarations.isEmpty()) {
                for (DirectScreenDeclaration declaration : declarations) {
                    boolean owo = declaration.parent().startsWith("BaseOwoScreen<");
                    if (owo) {
                        assertEquals("BaseOwoScreen<FlowLayout>", declaration.parent(),
                            "a new direct BaseOwoScreen root must be classified explicitly before migration: "
                                + relative + "#" + declaration.className());
                    }
                    result.add(new ScreenInventoryRow(
                        relative,
                        declaration.className(),
                        owo ? "BASE_OWO" : "VANILLA_SCREEN",
                        owo ? adapterStyle(path) : "VANILLA",
                        owo,
                        noteFor(relative)
                    ));
                }
            } else if (path.getFileName().toString().endsWith("Screen.java")) {
                String simpleName = path.getFileName().toString().replaceFirst("\\.java$", "");
                result.add(new ScreenInventoryRow(
                    relative,
                    simpleName,
                    "NON_SCREEN_HELPER",
                    "NONE",
                    false,
                    noteFor(relative)
                ));
            }
        }
        result.sort((left, right) -> {
            boolean leftLegacy = left.path().equals("cultivation/voidaction/LegacyAssignPanel.java");
            boolean rightLegacy = right.path().equals("cultivation/voidaction/LegacyAssignPanel.java");
            if (leftLegacy != rightLegacy) {
                return leftLegacy ? 1 : -1;
            }
            return left.path().compareTo(right.path());
        });
        return result;
    }

    private static List<Path> productionJavaFiles() throws IOException {
        try (var files = Files.walk(PRODUCTION_ROOT)) {
            return files.filter(Files::isRegularFile)
                .filter(candidate -> candidate.getFileName().toString().endsWith(".java"))
                .sorted()
                .toList();
        }
    }

    private static List<DirectScreenDeclaration> directScreenDeclarations(Path path) throws IOException {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        assertNotNull(compiler, "R7 Screen inventory requires a full Java 17 JDK, not a JRE");
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
            List<DirectScreenDeclaration> declarations = new ArrayList<>();
            for (CompilationUnitTree unit : task.parse()) {
                new TreePathScanner<Void, Void>() {
                    @Override
                    public Void visitClass(ClassTree classTree, Void unused) {
                        if (classTree.getExtendsClause() != null) {
                            String parent = classTree.getExtendsClause().toString().replaceAll("\\s+", "");
                            if (parent.equals("Screen") || parent.startsWith("BaseOwoScreen<")) {
                                declarations.add(new DirectScreenDeclaration(
                                    classTree.getSimpleName().toString(),
                                    parent
                                ));
                            }
                        }
                        return super.visitClass(classTree, unused);
                    }
                }.scan(unit, null);
            }
            assertTrue(diagnostics.getDiagnostics().stream()
                    .noneMatch(diagnostic -> diagnostic.getKind() == javax.tools.Diagnostic.Kind.ERROR),
                "unable to parse production Screen source " + path + ": " + diagnostics.getDiagnostics());
            return declarations;
        }
    }

    private record DirectScreenDeclaration(String className, String parent) {
    }

    private static String adapterStyle(Path path) throws IOException {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        assertNotNull(compiler, "R7 adapter classification requires a full Java 17 JDK, not a JRE");
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
            List<AdapterInvocation> invocations = new ArrayList<>();
            for (CompilationUnitTree unit : task.parse()) {
                collectAdapterInvocations(unit, invocations);
            }
            assertTrue(diagnostics.getDiagnostics().stream()
                    .noneMatch(diagnostic -> diagnostic.getKind() == javax.tools.Diagnostic.Kind.ERROR),
                "unable to parse production adapter source " + path + ": " + diagnostics.getDiagnostics());
            assertEquals(1, invocations.size(),
                "each direct owo Screen must expose one unambiguous adapter factory call in createAdapter(): " + path);
            return classifyAdapterInvocation(invocations.get(0));
        }
    }

    private static ExpressionTree unwrapReceiver(ExpressionTree receiver) {
        ExpressionTree current = receiver;
        while (true) {
            if (current instanceof ParenthesizedTree parenthesized) {
                current = parenthesized.getExpression();
            } else if (current instanceof TypeCastTree cast) {
                current = cast.getExpression();
            } else {
                return current;
            }
        }
    }

    private static String classifyAdapterInvocation(AdapterInvocation invocation) {
        if (invocation.method().equals("createAdapter")
            && invocation.arguments().equals(List.of("FlowLayout.class", "this"))) {
            return "XML_MODEL";
        }
        if (invocation.method().equals("create") && invocation.arguments().size() == 2) {
            return "CODE";
        }
        throw new AssertionError("unclassified owo adapter semantics: " + invocation);
    }

    @Test
    void adapterClassifierUsesInvocationSemanticsAcrossReceiverExpressions() {
        assertEquals("XML_MODEL", adapterStyleFromSource("""
            class Probe {
                Object createAdapter() {
                    return model.createAdapter(FlowLayout.class, this);
                }
            }
            """));
        assertEquals("XML_MODEL", adapterStyleFromSource("""
            class Probe {
                Object createAdapter() {
                    return provider().createAdapter(FlowLayout.class, this);
                }
            }
            """));
        assertEquals("XML_MODEL", adapterStyleFromSource("""
            class Probe {
                Object createAdapter() {
                    return holder.current.createAdapter(FlowLayout.class, this);
                }
            }
            """));
        assertEquals("CODE", adapterStyleFromSource("""
            class Probe {
                Object createAdapter() {
                    return OwoUIAdapter.create(this, Containers::verticalFlow);
                }
            }
            """));
        assertThrows(AssertionError.class, () -> adapterStyleFromSource("""
            class Probe {
                Object createAdapter() {
                    return model.createAdapter(GridLayout.class, this);
                }
            }
            """));
        assertThrows(AssertionError.class, () -> adapterStyleFromSource("""
            class Probe {
                Object createAdapter() {
                    return model.createAdapter(FlowLayout.class);
                }
            }
            """));
        assertThrows(AssertionError.class, () -> adapterStyleFromSource("""
            class Probe {
                Object createAdapter() {
                    return OwoUIAdapter.create(this);
                }
            }
            """));
        assertThrows(AssertionError.class, () -> adapterStyleFromSource("""
            class Probe {
                Object createAdapter() {
                    return OwoUIAdapter.create(this, Containers::verticalFlow, extra);
                }
            }
            """));
    }

    private static String adapterStyleFromSource(String source) {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        assertNotNull(compiler, "R7 adapter scanner test requires a full Java 17 JDK, not a JRE");
        DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
        JavaFileObject sourceFile = new SimpleJavaFileObject(
            java.net.URI.create("string:///Probe.java"),
            JavaFileObject.Kind.SOURCE
        ) {
            @Override
            public CharSequence getCharContent(boolean ignoreEncodingErrors) {
                return source;
            }
        };
        JavacTask task = (JavacTask) compiler.getTask(
            null, null, diagnostics, List.of("-proc:none"), null, List.of(sourceFile)
        );
        List<AdapterInvocation> invocations = new ArrayList<>();
        try {
            for (CompilationUnitTree unit : task.parse()) {
                collectAdapterInvocations(unit, invocations);
            }
        } catch (IOException exception) {
            throw new AssertionError("unable to parse adapter scanner test source", exception);
        }
        assertTrue(diagnostics.getDiagnostics().stream()
                .noneMatch(diagnostic -> diagnostic.getKind() == javax.tools.Diagnostic.Kind.ERROR),
            "unable to parse adapter scanner test source: " + diagnostics.getDiagnostics());
        assertEquals(1, invocations.size(), "scanner fixture must expose one adapter factory call");
        return classifyAdapterInvocation(invocations.get(0));
    }

    private static void collectAdapterInvocations(
        CompilationUnitTree unit,
        List<AdapterInvocation> invocations
    ) {
        new TreePathScanner<Void, Void>() {
            private boolean inCreateAdapter;

            @Override
            public Void visitMethod(MethodTree methodTree, Void unused) {
                boolean previous = inCreateAdapter;
                inCreateAdapter = methodTree.getName().contentEquals("createAdapter")
                    && methodTree.getParameters().isEmpty();
                try {
                    return super.visitMethod(methodTree, unused);
                } finally {
                    inCreateAdapter = previous;
                }
            }

            @Override
            public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                if (inCreateAdapter && invocation.getMethodSelect() instanceof MemberSelectTree select) {
                    String method = select.getIdentifier().toString();
                    if (method.equals("createAdapter") || method.equals("create")) {
                        invocations.add(new AdapterInvocation(
                            method,
                            unwrapReceiver(select.getExpression()).getKind().name(),
                            invocation.getArguments().stream()
                                .map(argument -> argument.toString().replaceAll("\\s+", ""))
                                .toList()
                        ));
                    }
                }
                return super.visitMethodInvocation(invocation, unused);
            }
        }.scan(unit, null);
    }

    private record AdapterInvocation(String method, String receiverKind, List<String> arguments) {
    }

    private static String noteFor(String path) {
        return switch (path) {
            case "agentui/AgentUiScreen.java" -> "UIModel adapter; base must not hard-code a root factory";
            case "alchemy/AlchemyScreen.java" -> "Code-built FlowLayout";
            case "coffin/CoffinMenuScreen.java" -> "Vanilla Screen, not a direct base migration";
            case "combat/screen/DeathScreen.java", "combat/screen/TerminateScreen.java" -> "System-terminal screen";
            case "combat/screen/ForgeCarrierScreen.java", "combat/screen/RepairScreen.java",
                "combat/screen/ZhenfaLayoutScreen.java", "cultivation/voidaction/VoidActionScreen.java",
                "forge/ForgeScreen.java", "identity/IdentityPanelScreen.java", "inspect/ItemInspectScreen.java",
                "spirittreasure/SpiritTreasureScreen.java" -> "Vanilla Screen";
            case "craft/CraftScreen.java", "craft/WorkbenchScreen.java", "inventory/LootContainerScreen.java",
                "lingtian/LingtianActionScreen.java", "npc/NpcDialogueScreen.java", "npc/NpcInspectScreen.java",
                "npc/NpcTradeScreen.java", "processing/ProcessingActionScreen.java", "scroll/ScrollReadScreen.java",
                "ui/CultivationScreen.java" -> "Code-built FlowLayout";
            case "cultivation/TechniqueScrollReadScreen.java" ->
                "Suffix matches Screen.java but class is a toast/text helper";
            case "insight/InsightOfferScreen.java" -> "Code-built modal FlowLayout";
            case "inventory/InspectScreen.java" -> "Code-built FlowLayout; P3 split target";
            case "social/SparringInviteScreen.java", "social/TradeOfferScreen.java" -> "Vanilla modal screen";
            case "ui/DynamicXmlScreen.java" -> "UIModel adapter; base must not hard-code a root factory";
            case "cultivation/voidaction/LegacyAssignPanel.java" ->
                "Real Screen missed by the Screen.java suffix inventory";
            default -> throw new AssertionError("fixture note mapping missing for " + path);
        };
    }

    private static long count(List<ScreenInventoryRow> rows, String kind) {
        return rows.stream().filter(row -> row.kind().equals(kind)).count();
    }

    private static Map<String, Long> histogram(List<String> values) {
        Map<String, Long> result = new TreeMap<>();
        for (String value : values) {
            result.merge(value, 1L, Long::sum);
        }
        return result;
    }

    private static List<ScreenInventoryRow> readScreenInventory() {
        return resourceLines("/bong/ui/r7-screen-inventory.tsv").stream()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new ScreenInventoryRow(
                columns[0],
                columns[1],
                columns[2],
                columns[3],
                Boolean.parseBoolean(columns[4]),
                columns[5]
            ))
            .toList();
    }

    private static List<FillInventoryRow> readFillInventory() {
        return resourceLines("/bong/ui/r7-fill100-inventory.tsv").stream()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new FillInventoryRow(
                columns[0],
                Integer.parseInt(columns[1]),
                Integer.parseInt(columns[2]),
                columns[3],
                columns[4],
                columns[5]
            ))
            .toList();
    }

    private static List<R7SourceScan.StructuralTokenOccurrence> readFillStructuralContext() {
        return resourceLines("/bong/ui/r7-fill100-structural-context.tsv").stream()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new R7SourceScan.StructuralTokenOccurrence(
                columns[0],
                columns[1],
                columns[2],
                columns[3]
            ))
            .toList();
    }

    private static List<String> resourceLines(String name) {
        try {
            var resource = R7InventoryContractTest.class.getResource(name);
            assertNotNull(resource, "missing R7 fixture " + name);
            return Files.readAllLines(Path.of(resource.toURI())).stream()
                .filter(R7SourceScan::isFixtureDataLine)
                .toList();
        } catch (IOException | URISyntaxException exception) {
            throw new AssertionError("unable to read R7 fixture " + name, exception);
        }
    }

    private static String relative(Path path) {
        return PRODUCTION_ROOT.relativize(path).toString().replace('\\', '/');
    }

    private static List<String> expectedFillClassifications() {
        return """
            alchemy/AlchemyScreen.java#1\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#2\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#3\tRISK\tEVICTS_LATER_SIBLING
            alchemy/AlchemyScreen.java#4\tRISK\tEVICTS_LATER_SIBLING
            alchemy/AlchemyScreen.java#5\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#6\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#7\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#8\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#9\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#10\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#11\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#12\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#13\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#14\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#15\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#16\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#17\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#18\tRISK\tEVICTS_LATER_SIBLING
            alchemy/AlchemyScreen.java#19\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#20\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#21\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#22\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#23\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#24\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#25\tRISK\tEVICTS_LATER_SIBLING
            alchemy/AlchemyScreen.java#26\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#27\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#28\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#29\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#30\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#31\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#32\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#33\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#34\tRISK\tTERMINAL_ORDER_DEPENDENT
            alchemy/AlchemyScreen.java#35\tLEGAL\tNONE
            alchemy/AlchemyScreen.java#36\tLEGAL\tNONE
            combat/inspect/SkillConfigFloatingWindow.java#1\tLEGAL\tNONE
            combat/inspect/SkillConfigFloatingWindow.java#2\tLEGAL\tNONE
            combat/inspect/SkillConfigFloatingWindow.java#3\tLEGAL\tNONE
            combat/inspect/TechniqueRowComponent.java#1\tLEGAL\tNONE
            combat/inspect/TechniquesTabPanel.java#1\tLEGAL\tNONE
            combat/inspect/TechniquesTabPanel.java#2\tLEGAL\tNONE
            craft/CraftActionBar.java#1\tLEGAL\tNONE
            craft/CraftActionBar.java#2\tCOMMENT\tNONE
            craft/CraftActionBar.java#3\tLEGAL\tTERMINAL_INTENTIONAL
            craft/CraftMaterialGrid.java#1\tLEGAL\tNONE
            craft/CraftMaterialGrid.java#2\tLEGAL\tNONE
            craft/CraftOutputPreview.java#1\tLEGAL\tNONE
            craft/CraftProgressBar.java#1\tLEGAL\tNONE
            craft/CraftProgressBar.java#2\tLEGAL\tNONE
            craft/CraftProgressBar.java#3\tLEGAL\tNONE
            craft/CraftRecipeListWidget.java#1\tLEGAL\tNONE
            craft/CraftRecipeListWidget.java#2\tLEGAL\tNONE
            craft/CraftRecipeListWidget.java#3\tCOMMENT\tNONE
            craft/CraftRecipeListWidget.java#4\tLEGAL\tNONE
            craft/CraftRecipeListWidget.java#5\tLEGAL\tNONE
            craft/CraftRecipeListWidget.java#6\tLEGAL\tNONE
            craft/CraftRecipeListWidget.java#7\tLEGAL\tNONE
            craft/CraftRecipeListWidget.java#8\tLEGAL\tNONE
            craft/CraftScreen.java#1\tLEGAL\tNONE
            craft/CraftScreen.java#2\tLEGAL\tNONE
            craft/CraftScreen.java#3\tLEGAL\tTERMINAL_INTENTIONAL
            craft/CraftScreenLayout.java#1\tCOMMENT\tNONE
            craft/WorkbenchScreen.java#1\tLEGAL\tNONE
            craft/WorkbenchScreen.java#2\tLEGAL\tNONE
            craft/WorkbenchScreen.java#3\tLEGAL\tTERMINAL_INTENTIONAL
            inventory/BlockPickerPanel.java#1\tLEGAL\tNONE
            inventory/BlockPickerPanel.java#2\tLEGAL\tNONE
            inventory/InspectScreen.java#1\tLEGAL\tNONE
            inventory/InspectScreen.java#2\tLEGAL\tNONE
            inventory/InspectScreen.java#3\tLEGAL\tNONE
            inventory/InspectScreen.java#4\tLEGAL\tNONE
            inventory/InspectScreen.java#5\tLEGAL\tNONE
            inventory/InspectScreen.java#6\tLEGAL\tNONE
            inventory/InspectScreen.java#7\tLEGAL\tNONE
            inventory/InspectScreen.java#8\tLEGAL\tNONE
            inventory/InspectScreen.java#9\tLEGAL\tNONE
            inventory/component/EquipmentPanel.java#1\tCOMMENT\tNONE
            lingtian/LingtianActionScreen.java#1\tLEGAL\tNONE
            lingtian/LingtianActionScreen.java#2\tLEGAL\tNONE
            lingtian/LingtianActionScreen.java#3\tLEGAL\tNONE
            lingtian/LingtianActionScreen.java#4\tLEGAL\tNONE
            lingtian/LingtianActionScreen.java#5\tLEGAL\tNONE
            lingtian/LingtianActionScreen.java#6\tLEGAL\tNONE
            lingtian/LingtianActionScreen.java#7\tLEGAL\tNONE
            lingtian/LingtianActionScreen.java#8\tLEGAL\tNONE
            npc/NpcTradeScreen.java#1\tLEGAL\tNONE
            processing/ProcessingActionScreen.java#1\tLEGAL\tNONE
            scroll/ScrollReadScreen.java#1\tCOMMENT\tNONE
            scroll/ScrollReadScreen.java#2\tLEGAL\tNONE
            scroll/ScrollReadScreen.java#3\tLEGAL\tNONE
            skill/SkillRowComponent.java#1\tLEGAL\tNONE
            """.strip().lines().toList();
    }

    private record ScreenInventoryRow(
        String path,
        String className,
        String kind,
        String adapterStyle,
        boolean eligible,
        String note
    ) {
    }

    private record FillInventoryRow(
        String path,
        int ordinal,
        int freezeLine,
        String verdict,
        String riskKind,
        String source
    ) {
        boolean code() {
            return !verdict.equals("COMMENT");
        }

        String stableKey() {
            return path + "#" + ordinal;
        }
    }
}
