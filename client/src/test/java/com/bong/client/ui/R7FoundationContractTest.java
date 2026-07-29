package com.bong.client.ui;

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
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class R7FoundationContractTest {
    private static final Path CLIENT_ROOT = R7SourceScan.productionRoot();
    private static final Path REPOSITORY_ROOT = R7SourceScan.repositoryRoot();
    private static final Path PLAN = REPOSITORY_ROOT.resolve("docs/plan-refactor-client-ui-base-v1.md");

    @Test
    void foundationFixturePinsFiveNamedContractsAndOwnership() {
        List<FoundationRow> rows = foundationRows();
        Map<String, Long> components = histogram(rows.stream().map(FoundationRow::component).toList());

        assertEquals(Set.of(
            "BongScreenBase",
            "BongKeybindRegistry",
            "ClientThreadMarshal",
            "DiffListWidget",
            "ScreenOpenPolicy"
        ), components.keySet(), "P0 freezes one base plus four shared helper contracts");
        assertEquals(34, rows.size(), "foundation signature inventory changed without an explicit P0 decision");
        assertTrue(rows.stream().allMatch(row -> row.owner().equals("R7")),
            "all five contract surfaces are R7-owned even when integration belongs to another track");
        assertTrue(rows.stream().anyMatch(row -> row.component().equals("BongScreenBase")
            && row.signature().contains("R extends ParentComponent")),
            "BongScreenBase must support both code-built and UIModel owo adapters");
        assertTrue(rows.stream().anyMatch(row -> row.component().equals("DiffListWidget")
            && row.symbol().equals("constructor")
            && row.signature().contains("Function<? super T, ? extends K> keyOf")),
            "a final DiffListWidget must receive its key extractor by constructor injection");
        assertFalse(rows.stream().anyMatch(row -> row.component().equals("DiffListWidget")
            && row.signature().contains("abstract")),
            "a final DiffListWidget cannot freeze abstract extension points");
        assertTrue(rows.stream().anyMatch(row -> row.component().equals("ClientThreadMarshal")
            && row.invariant().contains("false enqueues once")),
            "marshal contract must freeze exactly-once inline/enqueue behavior");
    }

    @Test
    void planCarriesTheFrozenContractAndBoundaryAnchors() {
        String plan = R7SourceScan.read(PLAN);
        for (String anchor : List.of(
            "29 个 production Screen",
            "92 个 `Sizing.fill(100)`",
            "BongScreenBase<R extends ParentComponent>",
            "DiffListWidget<T, K, C extends Component>",
            "BongKeybindRegistry",
            "ClientThreadMarshal",
            "ScreenOpenPolicy",
            "R2",
            "R6",
            "tab-first",
            "DEFER_NOTIFY",
            "BLOCK_DROP",
            "ZERO production behavior change"
        )) {
            assertTrue(plan.contains(anchor), "R7 P0 plan is missing frozen anchor: " + anchor);
        }
        assertTrue(plan.contains("`ClientThreadMarshal` 只冻结纯 helper API"),
            "R7 must not claim R6 network/router wiring");
        assertTrue(plan.contains("Screen-local listener/unsubscriber"),
            "Screen teardown must be distinguished from SessionScopedStore data clearing");
    }

    @Test
    void p0DoesNotTouchR2OrR6IntegrationSurfaces() {
        String plan = R7SourceScan.read(PLAN);
        assertFalse(plan.contains("基类要绑 SessionScopedStore"),
            "SessionScopedStore is a disconnect clearer, not a Screen subscription abstraction");

        String networkHandler = R7SourceScan.read(CLIENT_ROOT.resolve("BongNetworkHandler.java"));
        assertTrue(networkHandler.contains("clientExecutor.accept(() -> processServerDataPayload("),
            "R6-owned receive boundary must remain the production client-thread marshal owner");
        assertFalse(networkHandler.contains("ClientThreadMarshal"),
            "R7 P0 must not wire its helper into BongNetworkHandler");

        assertNoFoundationReference(CLIENT_ROOT.resolve("network"));
        assertNoFoundationReference(CLIENT_ROOT.resolve("BongNetworkHandler.java"));
    }

    @Test
    void keybindMigrationPinsCurrentAndConflictFreeTargetDefaults() {
        List<KeybindRow> rows = keybindRows();
        assertEquals(11, rows.size(), "R7 keybinding conflict cluster changed");
        assertEquals(8, rows.stream().filter(row -> row.targetDefault().equals("UNKNOWN")).count(),
            "five conflicting defaults plus the three dying-elder effective-binding entries remain unbound");
        assertEquals(3, rows.stream().filter(row -> row.conflict().equals("HUD_LABEL_MISMATCH")).count(),
            "dying-elder G/H/J are HUD effective-binding defects, not physical duplicate defaults");

        Set<String> knownTargets = Set.of("UNKNOWN", "O", "U", "R");
        assertTrue(rows.stream().allMatch(row -> knownTargets.contains(row.targetDefault())),
            "target defaults must use the explicitly frozen P1 set");
        assertEquals(expectedProductionKeySources().keySet(),
            rows.stream().map(KeybindRow::action).collect(java.util.stream.Collectors.toSet()),
            "every migration row must have one production declaration owner");
        for (KeybindRow row : rows) {
            Path source = CLIENT_ROOT.resolve(expectedProductionKeySources().get(row.action()));
            String sourceText = R7SourceScan.read(source);
            assertTrue(sourceText.contains("\"" + row.translationKey() + "\""),
                "migration translation key does not match production source " + source + ": "
                    + row.translationKey());
            assertKeyBindingDeclaration(sourceText, row, source);
        }

        Set<String> boundTargets = new TreeSet<>();
        for (KeybindRow row : rows) {
            if (!row.targetDefault().equals("UNKNOWN")) {
                assertTrue(boundTargets.add(row.targetDefault()),
                    "P1 target defaults must be physically unique: duplicate " + row.targetDefault());
            }
        }
        assertEquals(Set.of("O", "R", "U"), boundTargets);
        assertEquals("T", find(rows, "spirit_treasure_open").currentDefault());
        assertEquals("L", find(rows, "lingtian_open").currentDefault());
        assertEquals("UNKNOWN", find(rows, "dying_elder_give").targetDefault());
    }

    @Test
    void screenOpenDecisionTableFreezesDeferredInvitesAndDroppedHotkeys() {
        List<OpenPolicyRow> rows = openPolicyRows();
        assertEquals(14, rows.size(), "ScreenOpenPolicy P0 decision vectors changed");
        assertEquals("DEFER_NOTIFY", findPolicy(rows, "social-combat").decision());
        assertEquals("DEFER_NOTIFY", findPolicy(rows, "social-screen").decision());
        assertEquals("EXPIRE", findPolicy(rows, "social-expired").decision());
        assertEquals("BLOCK_DROP", findPolicy(rows, "hotkey-blocked").decision());
        assertEquals("PREEMPT", findPolicy(rows, "insight-preempt").decision());
        assertEquals("DEFER_NOTIFY", findPolicy(rows, "insight-terminal").decision());
        assertEquals("PREEMPT", findPolicy(rows, "terminal-preempt").decision());

        assertTrue(rows.stream()
            .filter(row -> row.requestKind().equals("HOTKEY"))
            .noneMatch(row -> row.decision().equals("DEFER_NOTIFY")),
            "physical keypresses must never be queued for later replay");
        assertTrue(rows.stream()
            .filter(row -> row.requestKind().equals("SOCIAL_INVITE") && row.unexpired())
            .allMatch(row -> Set.of("OPEN", "NOOP_MATCHING", "DEFER_NOTIFY").contains(row.decision())),
            "a live passive social offer remains in its authoritative domain store until open, match, or defer");
    }

    private static void assertNoFoundationReference(Path root) {
        List<String> names = List.of(
            "BongScreenBase",
            "DiffListWidget",
            "BongKeybindRegistry",
            "ClientThreadMarshal",
            "ScreenOpenPolicy"
        );
        try {
            if (Files.isRegularFile(root)) {
                String source = R7SourceScan.codeOnly(R7SourceScan.read(root));
                for (String name : names) {
                    assertFalse(source.contains(name), "P0/R6 ownership violation in " + root + ": " + name);
                }
                return;
            }
            try (var files = Files.walk(root)) {
                for (Path path : files.filter(Files::isRegularFile)
                    .filter(candidate -> candidate.getFileName().toString().endsWith(".java"))
                    .toList()) {
                    String source = R7SourceScan.codeOnly(R7SourceScan.read(path));
                    for (String name : names) {
                        assertFalse(source.contains(name), "P0/R6 ownership violation in " + path + ": " + name);
                    }
                }
            }
        } catch (IOException exception) {
            throw new AssertionError("unable to scan R7 ownership boundary " + root, exception);
        }
    }

    private static void assertKeyBindingDeclaration(String source, KeybindRow row, Path path) {
        String translationLiteral = "\"" + row.translationKey() + "\"";
        int literalIndex = source.indexOf(translationLiteral);
        assertTrue(literalIndex >= 0, "missing production translation literal in " + path + ": " + row.translationKey());

        String variable = declaredStringVariable(source, literalIndex);
        String translationArgument = variable == null ? translationLiteral : variable;
        int bindingIndex = findBindingUsing(source, translationArgument);
        assertTrue(bindingIndex >= 0,
            "translation key is not connected to a KeyBinding declaration in " + path + ": " + row.translationKey());

        int declarationEnd = source.indexOf(")", bindingIndex);
        assertTrue(declarationEnd > bindingIndex, "unterminated KeyBinding declaration in " + path);
        String declaration = source.substring(bindingIndex, declarationEnd + 1);
        assertTrue(containsCurrentDefault(source, declaration, row.currentDefault()),
            "migration key/default are not wired in the same KeyBinding declaration " + path + ": "
                + row.translationKey() + " -> " + row.currentDefault());
    }

    private static String declaredStringVariable(String source, int literalIndex) {
        int lineStart = source.lastIndexOf('\n', literalIndex) + 1;
        String prefix = source.substring(lineStart, literalIndex);
        var matcher = java.util.regex.Pattern.compile("String\\s+(\\w+)\\s*=\\s*$").matcher(prefix);
        return matcher.find() ? matcher.group(1) : null;
    }

    private static int findBindingUsing(String source, String argument) {
        int searchFrom = 0;
        while (true) {
            int bindingIndex = source.indexOf("new KeyBinding(", searchFrom);
            if (bindingIndex < 0) {
                return -1;
            }
            int declarationEnd = source.indexOf(")", bindingIndex);
            if (declarationEnd < 0) {
                return -1;
            }
            String declaration = source.substring(bindingIndex, declarationEnd + 1);
            if (declaration.contains(argument)) {
                return bindingIndex;
            }
            searchFrom = declarationEnd + 1;
        }
    }

    private static boolean containsCurrentDefault(String source, String declaration, String defaultKey) {
        if (defaultKey.equals("UNKNOWN")) {
            return declaration.contains("InputUtil.UNKNOWN_KEY.getCode()")
                || declaration.contains("GLFW.GLFW_KEY_UNKNOWN");
        }
        String direct = "GLFW.GLFW_KEY_" + defaultKey;
        if (declaration.contains(direct)) {
            return true;
        }
        for (String variable : List.of("DEFAULT_KEY", "DEFAULT_KEY_CODE")) {
            if (declaration.contains(variable)
                && java.util.regex.Pattern.compile(
                    "(?:int\\s+)?" + variable + "\\s*=\\s*" + java.util.regex.Pattern.quote(direct)
                ).matcher(source).find()) {
                return true;
            }
        }
        return false;
    }

    private static Map<String, String> expectedProductionKeySources() {
        return Map.ofEntries(
            Map.entry("spirit_treasure_open", "spirittreasure/SpiritTreasureScreenBootstrap.java"),
            Map.entry("lingtian_open", "lingtian/LingtianActionScreenBootstrap.java"),
            Map.entry("identity_open", "identity/IdentityPanelScreenBootstrap.java"),
            Map.entry("void_action_open", "cultivation/voidaction/VoidActionScreenBootstrap.java"),
            Map.entry("forge_open", "forge/ForgeScreenBootstrap.java"),
            Map.entry("extract_cancel", "tsy/ExtractInteractionBootstrap.java"),
            Map.entry("botany_auto", "botany/BotanyHudBootstrap.java"),
            Map.entry("spell_volume_hold", "combat/CombatKeybindings.java"),
            Map.entry("dying_elder_give", "dying_elder/DyingElderInteractionKeybindings.java"),
            Map.entry("dying_elder_refuse", "dying_elder/DyingElderInteractionKeybindings.java"),
            Map.entry("dying_elder_delay", "dying_elder/DyingElderInteractionKeybindings.java")
        );
    }

    private static KeybindRow find(List<KeybindRow> rows, String action) {
        return rows.stream()
            .filter(row -> row.action().equals(action))
            .findFirst()
            .orElseThrow(() -> new AssertionError("missing keybind row " + action));
    }

    private static OpenPolicyRow findPolicy(List<OpenPolicyRow> rows, String scenario) {
        return rows.stream()
            .filter(row -> row.scenario().equals(scenario))
            .findFirst()
            .orElseThrow(() -> new AssertionError("missing ScreenOpenPolicy row " + scenario));
    }

    private static Map<String, Long> histogram(List<String> values) {
        Map<String, Long> result = new TreeMap<>();
        for (String value : values) {
            result.merge(value, 1L, Long::sum);
        }
        return result;
    }

    private static List<FoundationRow> foundationRows() {
        return resourceLines("/bong/ui/r7-foundation-contract.tsv").stream()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new FoundationRow(columns[0], columns[1], columns[2], columns[3], columns[4]))
            .toList();
    }

    private static List<KeybindRow> keybindRows() {
        return resourceLines("/bong/ui/r7-keybind-migration.tsv").stream()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new KeybindRow(
                columns[0], columns[1], columns[2], columns[3], columns[4], columns[5]
            ))
            .toList();
    }

    private static List<OpenPolicyRow> openPolicyRows() {
        return resourceLines("/bong/ui/r7-screen-open-policy.tsv").stream()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new OpenPolicyRow(
                columns[0],
                columns[1],
                columns[2],
                Boolean.parseBoolean(columns[3]),
                Boolean.parseBoolean(columns[4]),
                Boolean.parseBoolean(columns[5]),
                columns[6],
                columns[7]
            ))
            .toList();
    }

    private static List<String> resourceLines(String name) {
        try {
            var resource = R7FoundationContractTest.class.getResource(name);
            assertNotNull(resource, "missing R7 fixture " + name);
            return Files.readAllLines(Path.of(resource.toURI())).stream()
                .filter(line -> !line.isBlank() && !line.startsWith("#"))
                .toList();
        } catch (IOException | URISyntaxException exception) {
            throw new AssertionError("unable to read R7 fixture " + name, exception);
        }
    }

    private record FoundationRow(String component, String symbol, String signature, String owner, String invariant) {
    }

    private record KeybindRow(
        String action,
        String translationKey,
        String currentDefault,
        String targetDefault,
        String conflict,
        String resolution
    ) {
    }

    private record OpenPolicyRow(
        String scenario,
        String requestKind,
        String currentKind,
        boolean combatActive,
        boolean matching,
        boolean unexpired,
        String decision,
        String rationale
    ) {
    }
}
