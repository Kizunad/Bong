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
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
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
        assertEquals(14, count(expectedRows, "BASE_OWO"), "direct legacy owo migration set changed");
        assertEquals(1, count(expectedRows, "OWO_XML"), "P2 owo XML host set changed");
        assertEquals(14, count(expectedRows, "VANILLA_SCREEN"), "direct vanilla Screen set changed");
        assertEquals(1, count(expectedRows, "NON_SCREEN_HELPER"), "Screen.java false-positive set changed");
        assertEquals(14, expectedRows.stream().filter(ScreenInventoryRow::eligible).count(),
            "P1 base migration is limited to direct legacy owo Screens");
        assertTrue(expectedRows.stream().anyMatch(row -> row.path().equals(
            "cultivation/voidaction/LegacyAssignPanel.java")),
            "suffix-only discovery must not lose a real Screen named LegacyAssignPanel");
        assertTrue(expectedRows.stream().anyMatch(row -> row.path().equals(
            "cultivation/TechniqueScrollReadScreen.java") && row.kind().equals("NON_SCREEN_HELPER")),
            "suffix-only discovery must not count TechniqueScrollReadScreen as a Screen");
    }

    @Test
    void fill100InventoryPinsExactRegistrationSites() throws IOException {
        List<FillInventoryRow> rows = readFillInventory();
        List<R7SourceScan.TokenOccurrence> actual = R7SourceScan.tokenOccurrences(PRODUCTION_ROOT, "Sizing.fill(100)");
        assertEquals(89, rows.size(), "the frozen fill inventory must enumerate every known occurrence");
        assertEquals(rows.stream().map(FillInventoryRow::stableKey).toList(),
            actual.stream().map(R7SourceScan.TokenOccurrence::stableKey).toList(),
            "the fixture must enumerate every production fill token in path-local order");
        assertEquals(rows.stream().map(FillInventoryRow::code).toList(),
            actual.stream().map(R7SourceScan.TokenOccurrence::code).toList(),
            "executable fill calls must be distinguished from raw comment or literal occurrences by the Java AST");
        assertEquals(rows.stream().map(FillInventoryRow::freezeLine).toList(),
            actual.stream().map(R7SourceScan.TokenOccurrence::line).toList(),
            "every frozen line must come from the production compilation unit line map");
        assertEquals(rows.stream().map(FillInventoryRow::source).toList(),
            actual.stream().map(R7SourceScan.TokenOccurrence::sourceLine).toList(),
            "every frozen source line must match production bytes");
        assertEquals(19, actual.stream().map(R7SourceScan.TokenOccurrence::path).distinct().count(),
            "the frozen fill inventory file set changed");
        assertEquals(Map.of("COMMENT", 5L, "LEGAL", 79L, "RISK", 5L),
            histogram(rows.stream().map(FillInventoryRow::verdict).toList()),
            "the frozen fill classification counts changed");
        assertEquals(expectedFillClassifications(), rows.stream()
                .map(row -> row.stableKey() + "\t" + row.verdict() + "\t" + row.riskKind())
                .toList(),
            "every exact fill registration site must be explicitly re-decided");

        List<R7SourceScan.StructuralTokenOccurrence> structural = readFillStructuralContext();
        assertEquals(structural, R7SourceScan.structuralTokenOccurrences(PRODUCTION_ROOT, "Sizing.fill(100)"),
            "every executable fill site must match its production enclosing class, method, and source hash");
        assertEquals(84, structural.size(),
            "all executable fill sites must carry one frozen structural context");
        assertEquals(84, structural.stream().map(R7SourceScan.StructuralTokenOccurrence::stableKey).distinct().count(),
            "structural-context stable keys must be unique");
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
    void p1ProductionSourceTreeMatchesFrozenBaseline() throws IOException {
        // 逐文件对拍确认 client/src/main 下只有已声明的生产变更，其余内容不动。
        //
        // 2026-08-30 收窄：**排除 resources/assets/**。
        //
        // 原范围是整个 client/src/main（2207 文件），其中 1139 个是 resources/assets，
        // 而那些字节已经被资源包 sha1 闸门逐字节钉住了——`client/resourcepack/manifest.json`
        // 与 `server/src/network/resourcepack.rs` 的 sha1 必须等于实际构建出的包，服务端
        // 还会把它通过 vanilla 协议下发给客户端校验。在这里再钉一遍买不到任何额外保证。
        //
        // 代价却是实打实的：这条基线被重新冻结过 21 次，且**每一次美术资产 PR 都要重来**。
        // 更糟的是它对合并顺序敏感——#2101 按规矩重新冻结了，仍然红，因为它算摘要时
        // #2113 的四张卷帛甲贴图还没并进来。两个各自都对的 PR，合起来就破。这个会反复发生。
        //
        // 收窄后本条的范围是 Java（1066）+ fabric.mod.json + bong-client.mixins.json（2）。
        // Java 那部分与 R7ProductionBaselineContractTest 重叠（那条只钉
        // java/com/bong/client），是本条原有的冗余，未在本次改动中拆开；本条独有的覆盖
        // 就是那两个 mod 配置文件，它们不进资源包，别处没人钉。
        String scope = "production-input-no-assets";
        assertEquals(
            R7SourceScan.baselineDigest(scope),
            R7SourceScan.sourceTreeDigest(PRODUCTION_INPUT_ROOT, R7SourceScan::isNotShippedAsset),
            () -> {
                long counted;
                try {
                    counted = R7SourceScan.treeFileCount(PRODUCTION_INPUT_ROOT, R7SourceScan::isNotShippedAsset);
                } catch (IOException exception) {
                    counted = -1;
                }
                return "R7 生产输入树漂移了（scope=" + scope + "，当前 " + counted + " 个文件，"
                    + "范围 = " + PRODUCTION_INPUT_ROOT + " 减去 " + R7SourceScan.shippedAssetRoot() + "）。\n"
                    + "这条基线只管**不随资源包发布**的生产字节：Java 生产源 + fabric.mod.json + mixins.json。\n"
                    + "撞红说明其中有文件被增/删/改。用 `git diff --name-status <上一个绿的 commit> -- "
                    + "client/src/main ':!client/src/main/resources/assets'` 看是哪些。\n"
                    + "确认这些变更该发布之后，把下面的 actual 值写进 "
                    + "client/src/test/resources/bong/ui/production-source-baseline.tsv 的 " + scope + " 行。\n"
                    + "（改了 resources/assets 不会撞这条——那些字节归资源包 sha1 闸门管。）";
            }
        );
    }

    @Test
    void frozenBaselineScopeIgnoresShippedAssetsAndCatchesEverythingElse() throws IOException {
        // 差分注入：把这条基线「该抓的」和「该放过的」都造出来，验证它分得清。
        //
        // 收窄范围这种改动最容易出的错是把判据改废了——它照样全绿，但已经什么都不抓了。
        // 某版穿模判据白名单写反、坏版本和修好的版本都报 17 处，就是这么来的。
        Path root = Files.createTempDirectory("r7-scope-injection");
        Path java = Files.createDirectories(root.resolve("java/com/bong/client"));
        Path assets = Files.createDirectories(root.resolve("resources/assets/bong/textures"));
        Path resources = root.resolve("resources");
        Files.writeString(java.resolve("Thing.java"), "class Thing {}");
        Files.writeString(assets.resolve("a.png"), "PNG-A");
        Files.writeString(resources.resolve("fabric.mod.json"), "{}");

        var include = R7SourceScan.excluding(assets.getParent().getParent());
        String baseline = R7SourceScan.sourceTreeDigest(root, include);

        // ── 该放过的：资产改了、资产新增了，都不该撞这条基线 ──
        Files.writeString(assets.resolve("a.png"), "PNG-A-CHANGED");
        assertEquals(baseline, R7SourceScan.sourceTreeDigest(root, include),
            "期望：改动 resources/assets 下的字节不撞本基线（那些字节由资源包 sha1 闸门钉住），"
            + "实际撞了 —— 收窄没生效，美术资产 PR 会继续被迫重新冻结");
        Files.writeString(assets.resolve("b.png"), "PNG-B");
        assertEquals(baseline, R7SourceScan.sourceTreeDigest(root, include),
            "期望：新增 resources/assets 下的文件不撞本基线；实际撞了");

        // ── 该抓的：Java 生产源与 mod 配置，一个都不许漏 ──
        Files.writeString(java.resolve("Thing.java"), "class Thing { int x; }");
        String afterJava = R7SourceScan.sourceTreeDigest(root, include);
        assertNotEquals(baseline, afterJava,
            "期望：改动 Java 生产源必须撞本基线，否则收窄把判据改废了；实际摘要没变");

        Files.writeString(java.resolve("Added.java"), "class Added {}");
        assertNotEquals(afterJava, R7SourceScan.sourceTreeDigest(root, include),
            "期望：新增 Java 生产源必须撞本基线；实际摘要没变");

        String beforeConfig = R7SourceScan.sourceTreeDigest(root, include);
        Files.writeString(resources.resolve("fabric.mod.json"), "{\"a\":1}");
        assertNotEquals(beforeConfig, R7SourceScan.sourceTreeDigest(root, include),
            "期望：改动 fabric.mod.json 必须撞本基线 —— 它不进资源包，本条是它唯一的看守；"
            + "实际摘要没变");
    }

    @Test
    void p1RetainsScreenInventoryWithoutLegacyFoundation() throws IOException {
        Set<String> forbiddenProductionTypes = Set.of(
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
        assertTrue(discovered.isEmpty(),
            "R7 P1 must not add an unapproved foundation type: " + discovered);
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
            case "agentui/AgentUiScreen.java" -> "UIModel adapter; base must not hard-code a root factory";
            case "alchemy/AlchemyScreen.java" -> "Code-built FlowLayout";
            case "coffin/CoffinMenuScreen.java" -> "Vanilla Screen, not a direct base migration";
            case "combat/screen/DeathScreen.java", "combat/screen/TerminateScreen.java" -> "System-terminal screen";
            case "combat/screen/ForgeCarrierScreen.java", "combat/screen/RepairScreen.java",
                "combat/screen/ZhenfaLayoutScreen.java", "cultivation/voidaction/VoidActionScreen.java",
                "forge/ForgeScreen.java", "identity/IdentityPanelScreen.java", "inspect/ItemInspectScreen.java",
                "spirittreasure/SpiritTreasureScreen.java" -> "Vanilla Screen";
            case "craft/CraftScreen.java" -> "P2 owo XML vertical slice";
            case "craft/WorkbenchScreen.java", "inventory/LootContainerScreen.java",
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

    private record DirectScreenDeclaration(String className, String parent) {
    }

    @Test
    void clearChildrenInventoryPinsExactProductionSites() throws IOException {
        List<String> sites = List.of(
            "alchemy/AlchemyScreen.java:538",
            "alchemy/AlchemyScreen.java:578",
            "alchemy/AlchemyScreen.java:607",
            "alchemy/AlchemyScreen.java:636",
            "combat/inspect/SkillConfigPanelManager.java:76",
            "combat/inspect/SkillConfigPanelManager.java:84",
            "combat/inspect/TechniquesTabPanel.java:149",
            "craft/CraftMaterialGrid.java:52",
            "craft/CraftMaterialGrid.java:53",
            "craft/CraftOutputPreview.java:32",
            "craft/CraftRecipeListWidget.java:134",
            "insight/InsightOfferScreen.java:107",
            "inventory/BlockPickerPanel.java:106",
            "inventory/InspectScreen.java:1685",
            "npc/NpcTradeScreen.java:163"
        );
        List<String> actual = R7SourceScan.zeroArgumentInvocationSites(PRODUCTION_ROOT, "clearChildren");
        assertEquals(15, sites.size(), "the frozen executable clearChildren inventory changed");
        assertEquals(sites.stream().sorted().toList(), actual,
            "the inventory must match every executable zero-argument production clearChildren call");
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

    private static List<FillInventoryRow> readFillInventory() {
        return resourceLines("/bong/ui/fill100-inventory.tsv").stream()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new FillInventoryRow(
                columns[0], Integer.parseInt(columns[1]), Integer.parseInt(columns[2]),
                columns[3], columns[4], columns[5]
            ))
            .toList();
    }

    private static List<R7SourceScan.StructuralTokenOccurrence> readFillStructuralContext() {
        return resourceLines("/bong/ui/fill100-structural-context.tsv").stream()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new R7SourceScan.StructuralTokenOccurrence(
                columns[0], columns[1], columns[2], columns[3]
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

    private static Map<String, Long> histogram(List<String> values) {
        Map<String, Long> result = new TreeMap<>();
        for (String value : values) {
            result.merge(value, 1L, Long::sum);
        }
        return result;
    }

    private static long count(List<ScreenInventoryRow> rows, String kind) {
        return rows.stream().filter(row -> row.kind().equals(kind)).count();
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
