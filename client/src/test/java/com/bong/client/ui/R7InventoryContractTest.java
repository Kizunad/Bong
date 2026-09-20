package com.bong.client.ui;

import com.sun.source.tree.ClassTree;
import com.sun.source.tree.ExpressionTree;
import com.sun.source.tree.MemberSelectTree;
import com.sun.source.tree.MethodInvocationTree;
import com.sun.source.tree.MethodTree;
import com.sun.source.tree.ParenthesizedTree;
import com.sun.source.tree.ReturnTree;
import com.sun.source.tree.TypeCastTree;
import com.sun.source.util.TreePathScanner;
import io.wispforest.owo.ui.core.Sizing;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.net.URISyntaxException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class R7InventoryContractTest {
    private static final Path PRODUCTION_ROOT = R7SourceScan.productionRoot();

    @Test
    void screenInventoryPinsEveryDirectProductionScreenAndSuffixException() throws IOException {
        List<ScreenInventoryRow> expectedRows = readScreenInventory();
        List<ScreenInventoryRow> actualRows = discoverDirectScreensAndSuffixHelpers();

        assertEquals(expectedRows, actualRows,
            "R7 Screen inventory drifted: every direct Screen and every *Screen.java false positive must be classified");
        assertEquals(28, expectedRows.size(), "物品详情与修仙概览已迁入窗口内容");
        assertEquals(13, count(expectedRows, "BASE_OWO"), "direct legacy owo migration set changed");
        assertEquals(9, count(expectedRows, "OWO_XML"), "owo XML host set changed");
        assertEquals(5, count(expectedRows, "VANILLA_SCREEN"), "direct vanilla Screen set changed");
        assertEquals(1, count(expectedRows, "NON_SCREEN_HELPER"), "Screen.java false-positive set changed");
        assertEquals(13, expectedRows.stream().filter(ScreenInventoryRow::eligible).count(),
            "P1 base migration is limited to direct legacy owo Screens");
        assertTrue(expectedRows.stream().anyMatch(row -> row.path().equals(
            "cultivation/TechniqueScrollReadScreen.java") && row.kind().equals("NON_SCREEN_HELPER")),
            "suffix-only discovery must not count TechniqueScrollReadScreen as a Screen");
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
    void p4AddsOnlyTheFrozenFoundationTypes() throws IOException {
        Set<String> expectedProductionTypes = Set.of(
            "ClientThreadMarshal.java",
            "ScreenOpenPolicy.java"
        );
        Set<String> discovered = new TreeSet<>();
        try (var files = Files.walk(PRODUCTION_ROOT)) {
            files.filter(Files::isRegularFile)
                .map(path -> path.getFileName().toString())
                .filter(expectedProductionTypes::contains)
                .forEach(discovered::add);
        }
        assertEquals(expectedProductionTypes, discovered,
            "P4 必须落地冻结的两个 foundation helper，不能漏实现或增添未审批类型");
        for (ScreenInventoryRow row : readScreenInventory()) {
            if (!row.kind().equals("BASE_OWO")) {
                continue;
            }
            String code = R7SourceScan.read(PRODUCTION_ROOT.resolve(row.path()));
            assertTrue(code.contains("extends BaseOwoScreen<FlowLayout>"),
                "P0 must not migrate production Screen inheritance: " + row.path());
        }
    }

    private static List<ScreenInventoryRow> discoverDirectScreensAndSuffixHelpers() throws IOException {
        List<ScreenInventoryRow> result = new java.util.ArrayList<>();
        for (R7SourceScan.ParsedUnit parsed : R7SourceScan.parseJava(PRODUCTION_ROOT)) {
            String relative = PRODUCTION_ROOT.relativize(parsed.path()).toString().replace('\\', '/');
            if (relative.startsWith("ui/adapter/owo/")) {
                continue;
            }
            List<DirectScreenDeclaration> declarations = new java.util.ArrayList<>();
            List<String> adapterStyles = new java.util.ArrayList<>();
            new TreePathScanner<Void, Void>() {
                private boolean inCreateAdapter;

                @Override
                public Void visitClass(ClassTree tree, Void unused) {
                    if (tree.getExtendsClause() != null) {
                        String parent = normalizeScreenParent(tree.getExtendsClause().toString());
                        if (parent.equals("Screen") || parent.startsWith("BaseOwoScreen<")
                            || parent.startsWith("OwoXmlScreenHost<")) {
                            declarations.add(new DirectScreenDeclaration(tree.getSimpleName().toString(), parent));
                        }
                    }
                    return super.visitClass(tree, unused);
                }

                @Override
                public Void visitMethod(MethodTree tree, Void unused) {
                    boolean previous = inCreateAdapter;
                    inCreateAdapter = tree.getName().contentEquals("createAdapter") && tree.getParameters().isEmpty();
                    try {
                        return super.visitMethod(tree, unused);
                    } finally {
                        inCreateAdapter = previous;
                    }
                }

                @Override
                public Void visitReturn(ReturnTree tree, Void unused) {
                    if (inCreateAdapter) {
                        adapterStyles.add(classifyReturnedAdapter(tree.getExpression()));
                    }
                    return super.visitReturn(tree, unused);
                }
            }.scan(parsed.unit(), null);
            if (!declarations.isEmpty()) {
                for (DirectScreenDeclaration declaration : declarations) {
                    boolean owo = declaration.parent().startsWith("BaseOwoScreen<");
                    boolean xmlOwo = declaration.parent().startsWith("OwoXmlScreenHost<");
                    if (owo) {
                        assertEquals("BaseOwoScreen<FlowLayout>", declaration.parent(),
                            "new direct owo roots require an explicit migration decision: " + relative);
                        assertEquals(1, adapterStyles.size(),
                            "each direct owo Screen needs one returned adapter factory: " + relative);
                    }
                    result.add(new ScreenInventoryRow(
                        relative,
                        declaration.className(),
                        owo ? "BASE_OWO" : xmlOwo ? "OWO_XML" : "VANILLA_SCREEN",
                        owo ? adapterStyles.get(0) : xmlOwo ? "OWO_XML_TEMPLATE" : "VANILLA",
                        owo,
                        noteFor(relative)
                    ));
                }
            } else if (parsed.path().getFileName().toString().endsWith("Screen.java")) {
                result.add(new ScreenInventoryRow(
                    relative,
                    parsed.path().getFileName().toString().replaceFirst("\\.java$", ""),
                    "NON_SCREEN_HELPER",
                    "NONE",
                    false,
                    noteFor(relative)
                ));
            }
        }
        result.sort(java.util.Comparator.comparing(ScreenInventoryRow::path));
        return result;
    }

    private static String normalizeScreenParent(String parent) {
        String normalized = parent.replaceAll("\\s+", "");
        if (normalized.equals("net.minecraft.client.gui.screen.Screen")) {
            return "Screen";
        }
        if (normalized.startsWith("io.wispforest.owo.ui.base.BaseOwoScreen<")) {
            return normalized.substring("io.wispforest.owo.ui.base.".length());
        }
        if (normalized.startsWith("com.bong.client.ui.adapter.owo.OwoXmlScreenHost<")) {
            return normalized.substring("com.bong.client.ui.adapter.owo.".length());
        }
        return normalized;
    }

    private static String classifyReturnedAdapter(ExpressionTree expression) {
        ExpressionTree unwrapped = expression;
        while (unwrapped instanceof ParenthesizedTree parenthesized
            || unwrapped instanceof TypeCastTree) {
            unwrapped = unwrapped instanceof ParenthesizedTree parenthesized
                ? parenthesized.getExpression()
                : ((TypeCastTree) unwrapped).getExpression();
        }
        if (!(unwrapped instanceof MethodInvocationTree invocation)
            || !(invocation.getMethodSelect() instanceof MemberSelectTree select)) {
            throw new AssertionError("createAdapter must return one direct factory invocation: " + expression);
        }
        List<String> arguments = invocation.getArguments().stream()
            .map(argument -> argument.toString().replaceAll("\\s+", ""))
            .toList();
        if (select.getIdentifier().contentEquals("createAdapter")
            && arguments.equals(List.of("FlowLayout.class", "this"))) {
            return "XML_MODEL";
        }
        if (select.getIdentifier().contentEquals("create") && arguments.size() == 2) {
            return "CODE";
        }
        throw new AssertionError("unclassified returned owo adapter factory: " + invocation);
    }

    private static String noteFor(String path) {
        return switch (path) {
            case "menu/MainMenuScreen.java" -> "Multiplayer-only title screen; local XML";
            case "agentui/AgentUiScreen.java" -> "UIModel adapter; base must not hard-code a root factory";
            case "alchemy/AlchemyScreen.java" -> "Code-built FlowLayout";
            case "coffin/CoffinMenuScreen.java" -> "P4 XML migration slice; G menu";
            case "combat/screen/DeathScreen.java" -> "P4 XML migration slice; system-terminal screen";
            case "combat/screen/TerminateScreen.java" -> "P4 XML migration slice; system-terminal screen";
            case "combat/screen/ZhenfaLayoutScreen.java" -> "P4 XML migration slice; 阵法布置";
            case "cultivation/voidaction/VoidActionScreen.java",
                "forge/ForgeScreen.java", "spirittreasure/SpiritTreasureScreen.java" -> "Vanilla Screen";
            case "identity/IdentityPanelScreen.java" -> "P4 XML migration slice; identity panel";
            case "combat/screen/ForgeCarrierScreen.java" -> "P4 XML migration slice;暗器注入";
            case "combat/screen/RepairScreen.java" -> "P4 XML migration slice; weapon repair";
            case "craft/CraftScreen.java" -> "P2 owo XML vertical slice";
            case "craft/WorkbenchScreen.java", "inventory/LootContainerScreen.java",
                "lingtian/LingtianActionScreen.java", "npc/NpcDialogueScreen.java", "npc/NpcInspectScreen.java",
                "npc/NpcTradeScreen.java", "processing/ProcessingActionScreen.java", "scroll/ScrollReadScreen.java" -> "Code-built FlowLayout";
            case "cultivation/TechniqueScrollReadScreen.java" ->
                "Suffix matches Screen.java but class is a toast/text helper";
            case "insight/InsightOfferScreen.java" -> "Code-built modal FlowLayout";
            case "inventory/InspectScreen.java" -> "Code-built FlowLayout; P3 split target";
            case "social/SparringInviteScreen.java", "social/TradeOfferScreen.java" -> "Vanilla modal screen";
            case "ui/DynamicXmlScreen.java" -> "UIModel adapter; base must not hard-code a root factory";
            default -> throw new AssertionError("fixture note mapping missing for " + path);
        };
    }

    private record DirectScreenDeclaration(String className, String parent) {
    }

    private static List<ScreenInventoryRow> readScreenInventory() {
        return resourceLines("/bong/ui/screen-inventory.tsv").stream()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new ScreenInventoryRow(
                columns[0], columns[1], columns[2], columns[3],
                Boolean.parseBoolean(columns[4]), columns[5]
            ))
            .toList();
    }

    private static List<String> resourceLines(String name) {
        try {
            var resource = R7InventoryContractTest.class.getResource(name);
            assertNotNull(resource, "missing R7 fixture " + name);
            return Files.readAllLines(Path.of(resource.toURI())).stream()
                .filter(R7SourceScan::isFixtureDataLine)
                .map(line -> line.replaceFirst("^\\d+\\t", ""))
                .toList();
        } catch (IOException | URISyntaxException exception) {
            throw new AssertionError("unable to read R7 fixture " + name, exception);
        }
    }

    private static long count(List<ScreenInventoryRow> rows, String kind) {
        return rows.stream().filter(row -> row.kind().equals(kind)).count();
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

}
