package com.bong.client.ui;

import com.sun.source.tree.AssignmentTree;
import com.sun.source.tree.BinaryTree;
import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.ExpressionTree;
import com.sun.source.tree.IdentifierTree;
import com.sun.source.tree.LiteralTree;
import com.sun.source.tree.MemberSelectTree;
import com.sun.source.tree.MethodInvocationTree;
import com.sun.source.tree.NewClassTree;
import com.sun.source.tree.Tree;
import com.sun.source.tree.VariableTree;
import com.sun.source.util.JavacTask;
import com.sun.source.util.TreePath;
import com.sun.source.util.TreePathScanner;
import org.junit.jupiter.api.Test;

import javax.tools.Diagnostic;
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
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class R7FoundationContractTest {
    private static final Path CLIENT_ROOT = R7SourceScan.productionRoot();
    private static final Path REPOSITORY_ROOT = R7SourceScan.repositoryRoot();
    private static final Path PLAN = REPOSITORY_ROOT.resolve("docs/plan-refactor-client-ui-base-v1.md");

    @Test
    void fixtureLoaderSkipsPlainAndOrdinalPrefixedHeaders() {
        assertFalse(R7SourceScan.isFixtureDataLine("# owner\tinput_type"));
        assertFalse(R7SourceScan.isFixtureDataLine("0\t# owner\tinput_type"));
        assertTrue(R7SourceScan.isFixtureDataLine("owner\tKEYSYM\tT"));
    }

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
        assertEquals(37, rows.size(), "foundation signature inventory changed without an explicit P0 decision");
        assertEquals(37, Set.copyOf(rows).size(), "each frozen contract row must be unique");
        assertEquals(37, rows.stream()
            .map(row -> row.component() + "::" + row.symbol())
            .collect(java.util.stream.Collectors.toSet()).size(),
            "each frozen contract symbol must have one unambiguous signature row");
        assertEquals(expectedFoundationRows(), rows,
            "foundation fixture drifted: every signature, owner, and invariant must be explicitly re-decided");
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

        Set<String> knownTypes = Set.of("KEYSYM");
        assertTrue(rows.stream().allMatch(row -> knownTypes.contains(row.currentType())
            && knownTypes.contains(row.targetType())),
            "all migration-cluster declarations must freeze their physical InputUtil.Type");
        assertTrue(rows.stream().allMatch(row -> row.currentType().equals(row.targetType())),
            "P0 must not silently change physical input type while freezing target key codes");
        assertEquals(Set.of("O", "R", "U"), rows.stream()
            .filter(row -> !row.targetDefault().equals("UNKNOWN"))
            .map(KeybindRow::targetDefault)
            .collect(java.util.stream.Collectors.toSet()));
        Map<String, String> expectedTargets = Map.ofEntries(
            Map.entry("spirit_treasure_open", "UNKNOWN"),
            Map.entry("lingtian_open", "UNKNOWN"),
            Map.entry("identity_open", "O"),
            Map.entry("void_action_open", "UNKNOWN"),
            Map.entry("forge_open", "U"),
            Map.entry("extract_cancel", "UNKNOWN"),
            Map.entry("botany_auto", "UNKNOWN"),
            Map.entry("spell_volume_hold", "R"),
            Map.entry("dying_elder_give", "UNKNOWN"),
            Map.entry("dying_elder_refuse", "UNKNOWN"),
            Map.entry("dying_elder_delay", "UNKNOWN")
        );
        assertEquals(expectedTargets, rows.stream().collect(java.util.stream.Collectors.toMap(
            KeybindRow::action, KeybindRow::targetDefault
        )), "every migration action must retain its exact target default");
        assertEquals(expectedProductionKeySources().keySet(),
            rows.stream().map(KeybindRow::action).collect(java.util.stream.Collectors.toSet()),
            "every migration row must have one production declaration owner");
        for (KeybindRow row : rows) {
            assertEquals(expectedProductionKeySources().get(row.action()), row.productionOwner(),
                "fixture must freeze the exact production declaration owner for " + row.action());
            Path source = CLIENT_ROOT.resolve(row.productionOwner());
            String sourceText = R7SourceScan.read(source);
            assertTrue(sourceText.contains("\"" + row.translationKey() + "\""),
                "migration translation key does not match production source " + source + ": "
                    + row.translationKey());
            assertKeyBindingDeclaration(sourceText, row, source);
        }

        assertEquals("Remove the passive automation default; drain queued presses while session/screen preconditions reject dispatch; prove no later replay.",
            find(rows, "botany_auto").resolution(),
            "botany rejection must drain queued presses and prohibit later replay");
        Map<String, String> expectedDyingElderResolutions = Map.of(
            "dying_elder_give",
            "HUD must resolve the effective binding instead of promising G; show unbound explicitly; the unified G router remains unique.",
            "dying_elder_refuse",
            "HUD must resolve the effective binding instead of promising H; show unbound explicitly; the unified G router remains unique.",
            "dying_elder_delay",
            "HUD must resolve the effective binding instead of promising J; show unbound explicitly; the unified G router remains unique."
        );
        Map<String, String> actualDyingElderResolutions = rows.stream()
            .filter(row -> row.action().startsWith("dying_elder_"))
            .collect(java.util.stream.Collectors.toMap(KeybindRow::action, KeybindRow::resolution));
        assertEquals(expectedDyingElderResolutions, actualDyingElderResolutions,
            "all three dying-elder actions must use effective bindings, explicit unbound display, and one G router");

        Set<PhysicalDefault> boundTargets = new TreeSet<>((left, right) -> {
            int type = left.type().compareTo(right.type());
            return type != 0 ? type : left.code().compareTo(right.code());
        });
        for (KeybindRow row : rows) {
            if (!row.targetDefault().equals("UNKNOWN")) {
                assertTrue(boundTargets.add(new PhysicalDefault(row.targetType(), row.targetDefault())),
                    "P1 target defaults must be physically unique: duplicate "
                        + row.targetType() + "+" + row.targetDefault());
            }
        }
        assertEquals(Set.of(
            new PhysicalDefault("KEYSYM", "O"),
            new PhysicalDefault("KEYSYM", "R"),
            new PhysicalDefault("KEYSYM", "U")
        ), boundTargets);
    }

    @Test
    void screenOpenDecisionTableFreezesDeferredInvitesAndDroppedHotkeys() {
        List<String> fixtureLines = resourceLines("/bong/ui/r7-screen-open-policy.tsv");
        assertEquals(expectedOpenPolicyFixtureLines(), fixtureLines,
            "all 32 raw policy vectors and every input/output field must be explicitly re-decided");
        List<OpenPolicyRow> rows = openPolicyRows();
        assertEquals(32, rows.size(), "ScreenOpenPolicy P0 decision vectors changed");
        assertEquals(32, rows.stream().map(OpenPolicyRow::scenario)
            .collect(java.util.stream.Collectors.toSet()).size(),
            "each ScreenOpenPolicy scenario name must be unique");
        Set<String> requestKinds = Set.of("SOCIAL_INVITE", "HOTKEY", "INSIGHT", "SYSTEM_TERMINAL");
        Set<String> currentKinds = Set.of("NONE", "ORDINARY", "MODAL", "SYSTEM_TERMINAL");
        Set<String> terminalPriorities = Set.of("NONE", "DEATH", "TERMINATE");
        Set<String> decisions = Set.of(
            "OPEN", "PREEMPT", "NOOP_MATCHING", "DEFER_NOTIFY", "DEFER_SILENT", "BLOCK_DROP", "EXPIRE"
        );
        assertTrue(rows.stream().allMatch(row -> requestKinds.contains(row.requestKind())),
            "every vector request_kind must instantiate the frozen RequestKind enum");
        assertTrue(rows.stream().allMatch(row -> currentKinds.contains(row.currentKind())),
            "every vector current_kind must instantiate the frozen CurrentKind enum");
        assertTrue(rows.stream().allMatch(row -> terminalPriorities.contains(row.requestPriority())
            && terminalPriorities.contains(row.currentPriority())),
            "every vector priority must instantiate the frozen TerminalPriority enum");
        assertTrue(rows.stream().allMatch(R7FoundationContractTest::rowsUseValidPriorities),
            "only SYSTEM_TERMINAL request/current kinds may carry DEATH or TERMINATE priority");
        assertTrue(rows.stream().allMatch(row -> decisions.contains(row.decision())),
            "every vector decision must instantiate the frozen Decision enum");
        assertTrue(rows.stream().allMatch(row -> row.decision().equals("EXPIRE") == row.expired()),
            "the raw nowMs/expiresAtMs boundary must derive every finite expiry result");
        assertTrue(rows.stream()
            .filter(row -> row.decision().equals("NOOP_MATCHING"))
            .allMatch(OpenPolicyRow::matching),
            "NOOP_MATCHING must derive from equal non-empty request/current identities");
        assertTrue(rows.stream()
            .filter(row -> !Set.of("NOOP_MATCHING", "EXPIRE").contains(row.decision()))
            .noneMatch(OpenPolicyRow::matching),
            "live matching identity rows must use NOOP_MATCHING before priority arbitration");
        OpenPolicyRow expiryBeforeMatching = findPolicy(rows, "social-expired-boundary");
        assertTrue(expiryBeforeMatching.expired() && expiryBeforeMatching.matching(),
            "the precedence vector must make expiry and identity matching true simultaneously");
        assertEquals("EXPIRE", expiryBeforeMatching.decision(),
            "finite expiry wins before matching when both predicates hold");
        assertEquals("EXPIRE", findPolicy(rows, "social-expired-boundary").decision());
        assertEquals("DEFER_NOTIFY", findPolicy(rows, "social-combat-first").decision());
        assertEquals("DEFER_SILENT", findPolicy(rows, "social-combat-repeat").decision());
        assertEquals("DEFER_NOTIFY", findPolicy(rows, "social-modal-first").decision());
        assertEquals("DEFER_SILENT", findPolicy(rows, "social-modal-repeat").decision());
        assertEquals("DEFER_NOTIFY", findPolicy(rows, "social-new-identity").decision());
        assertEquals("BLOCK_DROP", findPolicy(rows, "hotkey-ordinary").decision());
        assertEquals("OPEN", findPolicy(rows, "insight-open").decision());
        assertEquals("EXPIRE", findPolicy(rows, "insight-expired").decision());
        assertEquals("PREEMPT", findPolicy(rows, "death-preempt-modal").decision());
        assertEquals("NOOP_MATCHING", findPolicy(rows, "death-matching").decision());
        assertEquals("PREEMPT", findPolicy(rows, "terminate-preempt-death").decision());
        assertEquals("BLOCK_DROP", findPolicy(rows, "death-blocked-terminate").decision());
        assertEquals("BLOCK_DROP", findPolicy(rows, "death-blocked-peer").decision());
        assertEquals("BLOCK_DROP", findPolicy(rows, "terminate-blocked-peer").decision());

        assertTrue(rows.stream()
            .filter(row -> row.requestKind().equals("HOTKEY"))
            .allMatch(row -> !Set.of("DEFER_NOTIFY", "DEFER_SILENT").contains(row.decision())),
            "physical keypresses must never be queued for later replay");
        assertTrue(rows.stream()
            .filter(row -> Set.of("SOCIAL_INVITE", "INSIGHT").contains(row.requestKind()))
            .filter(row -> Set.of("DEFER_NOTIFY", "DEFER_SILENT").contains(row.decision()))
            .allMatch(row -> row.alreadyNotified() == row.decision().equals("DEFER_SILENT")),
            "caller-owned notification state must distinguish first notification from silent repeated defer");
    }

    @Test
    void insightSettlementFixtureFreezesEveryTerminalCauseAndOwner() {
        List<String> lines = resourceLines("/bong/ui/r7-insight-settlement.tsv");
        List<InsightSettlementRow> rows = insightSettlementRows();
        assertEquals(8, rows.size(), "each bounded insight terminal path must have one contract row");
        assertEquals(expectedInsightSettlementRows(), rows,
            "every insight terminal cause, decision, owner, ordering, failure, and observable effect must be exact-pinned");
        assertEquals(Set.of(
            "ACCEPT", "DECLINE", "TIMEOUT", "ESC", "REPLACED_BY_DIFFERENT_OFFER",
            "ANIMATED_OPEN_CANCELLED", "REMOVED_EXCEPTIONALLY", "DUPLICATE_TERMINAL"
        ), rows.stream().map(InsightSettlementRow::terminalCause)
            .collect(java.util.stream.Collectors.toSet()),
            "every insight terminal cause must have one contract row");
        assertEquals(8, rows.stream().map(InsightSettlementRow::terminalCause).distinct().count(),
            "terminal causes must be unique");
        assertTrue(rows.stream().allMatch(row -> row.identityRule().contains("offerId")),
            "settlement identity must use wire offerId while triggerId remains reusable context");
        assertTrue(lines.stream().anyMatch(line -> line.contains("offerId") && line.contains("triggerId")),
            "fixture must distinguish reusable trigger context from unique offer instance identity");
    }

    @Test
    void keybindManifestsPinReservedDefaultsExemptionsAndEveryProductionSite() throws IOException {
        assertEquals(List.of(
            new ReservedDefaultRow("vanilla.chat", "KEYSYM", "T",
                "Minecraft chat default is reserved before Bong registrations."),
            new ReservedDefaultRow("vanilla.advancements", "KEYSYM", "L",
                "Minecraft advancements default is reserved before Bong registrations.")
        ), reservedDefaultRows(), "vanilla reservation manifest drifted");
        assertEquals(List.of(), conflictExemptionRows(), "physical-default exemptions must remain empty");

        List<KeybindProductionSiteRow> expected = keybindProductionSiteRows();
        List<KeybindingSourceSite> actualSites = productionKeybindingSourceSites();
        assertEquals(26, actualSites.size(), "production KeyBinding constructor-site count changed");
        assertEquals(26, expected.size(), "each constructor source site must have one exact binding contract");
        assertEquals(26, expected.stream().map(KeybindProductionSiteRow::ownerId)
            .collect(java.util.stream.Collectors.toSet()).size(),
            "every logical binding needs one globally unique BindingOwner id");
        Set<SourceSiteIdentity> expectedIdentities = expected.stream()
            .map(row -> new SourceSiteIdentity(row.sourcePath(), row.sourceSite()))
            .collect(java.util.stream.Collectors.toCollection(TreeSet::new));
        Set<SourceSiteIdentity> actualIdentities = actualSites.stream()
            .map(site -> new SourceSiteIdentity(site.sourcePath(), site.sourceSite()))
            .collect(java.util.stream.Collectors.toCollection(TreeSet::new));
        assertEquals(expectedIdentities, actualIdentities,
            "fixture and production must match the exact (sourcePath, assignment target) set in both directions");
        assertEquals(resourceLines("/bong/ui/r7-keybind-production-sites.tsv"),
            keybindProductionSiteRows().stream().map(KeybindProductionSiteRow::fixtureLine).toList(),
            "every production keybinding declaration must parse as one exact typed manifest row");
        assertEquals(34, actualSites.stream().mapToInt(KeybindingSourceSite::runtimeCardinality).sum(),
            "the AST-derived 26 constructors must expand to exactly 34 runtime bindings");
        Set<String> expandedTranslationKeys = new TreeSet<>();
        for (KeybindingSourceSite site : actualSites) {
            expandedTranslationKeys.addAll(site.expandedTranslationKeys());
        }
        assertEquals(34, expandedTranslationKeys.size(),
            "every AST-resolved runtime binding must have a unique effective translation key");
        for (KeybindProductionSiteRow row : expected) {
            KeybindingSourceSite actual = actualSites.stream()
                .filter(site -> site.sourcePath().equals(row.sourcePath())
                    && site.sourceSite().equals(row.sourceSite()))
                .findFirst()
                .orElseThrow(() -> new AssertionError("missing AST source site for " + row.ownerId()));
            assertEquals(row.translationContract(), actual.translationContract(),
                "effective translation contract drifted for " + row.ownerId());
            assertEquals(row.inputType(), actual.inputType(),
                "effective input type drifted for " + row.ownerId());
            assertEquals(row.normalizedDefaultContract(), actual.defaultContract(),
                "effective default key drifted for " + row.ownerId());
            assertEquals(row.categoryContract(), actual.categoryContract(),
                "effective category drifted for " + row.ownerId());
            assertEquals(row.runtimeCardinalityCount(), actual.runtimeCardinality(),
                "AST-derived runtime cardinality drifted for " + row.ownerId());
            assertEquals(row.expandedTranslationKeys(), actual.expandedTranslationKeys(),
                "expanded runtime translation keys drifted for " + row.ownerId());
        }
        List<ExpandedProductionDefault> expandedDefaults = expandedProductionDefaults(
            expected, actualSites, keybindRows()
        );
        assertEquals(34, expandedDefaults.size(),
            "the collision audit must inspect every expanded runtime production default, including UNKNOWN entries");
        Set<DefaultCollision> collisions = new TreeSet<>();
        for (int first = 0; first < expandedDefaults.size(); first++) {
            ExpandedProductionDefault left = expandedDefaults.get(first);
            if (left.defaultCode().equals("UNKNOWN")) {
                continue;
            }
            for (int second = first + 1; second < expandedDefaults.size(); second++) {
                ExpandedProductionDefault right = expandedDefaults.get(second);
                if (left.inputType().equals(right.inputType())
                    && left.defaultCode().equals(right.defaultCode())) {
                    collisions.add(new DefaultCollision(
                        left.ownerId(), right.ownerId(), left.inputType(), left.defaultCode()
                    ));
                }
            }
        }
        for (ExpandedProductionDefault production : expandedDefaults) {
            if (production.defaultCode().equals("UNKNOWN")) {
                continue;
            }
            for (ReservedDefaultRow reserved : reservedDefaultRows()) {
                if (production.inputType().equals(reserved.inputType())
                    && production.defaultCode().equals(reserved.code())) {
                    collisions.add(new DefaultCollision(
                        production.ownerId(), reserved.owner(), reserved.inputType(), reserved.code()
                    ));
                }
            }
        }
        Set<DefaultCollision> expectedCollisions = Set.of();
        assertEquals(expectedCollisions, collisions,
            "the 34 expanded target defaults must not collide with frozen vanilla reservations");

        Set<DefaultCollision> exemptions = conflictExemptionRows().stream()
            .map(row -> new DefaultCollision(row.firstOwnerId(), row.secondOwnerId(), row.inputType(), row.code()))
            .collect(java.util.stream.Collectors.toSet());
        assertEquals(collisions, exemptions,
            "every detected collision needs one exact owner/type/code exemption and no stale exemption may remain");
        assertTrue(conflictExemptionRows().stream().allMatch(row -> !row.reason().isBlank()),
            "every physical-default exemption must carry an actionable reason");
    }

    private static List<ExpandedProductionDefault> expandedProductionDefaults(
        List<KeybindProductionSiteRow> expected,
        List<KeybindingSourceSite> actualSites,
        List<KeybindRow> migrationRows
    ) {
        List<ExpandedProductionDefault> result = new java.util.ArrayList<>();
        for (KeybindProductionSiteRow row : expected) {
            KeybindingSourceSite actual = actualSites.stream()
                .filter(site -> site.sourcePath().equals(row.sourcePath())
                    && site.sourceSite().equals(row.sourceSite()))
                .findFirst()
                .orElseThrow(() -> new AssertionError("missing AST source site for " + row.ownerId()));
            List<String> owners = row.ownerId().equals("combat.quick_slot")
                ? java.util.stream.IntStream.rangeClosed(1, actual.runtimeCardinality())
                    .mapToObj(index -> "combat.quick_slot_" + index)
                    .toList()
                : List.of(row.ownerId());
            String effectiveDefaultContract = effectiveDefaultContract(row, migrationRows);
            List<String> defaults = expandedDefaultCodes(effectiveDefaultContract, actual.runtimeCardinality());
            assertEquals(owners.size(), defaults.size(),
                "expanded owner/default cardinality must agree for " + row.ownerId());
            for (int index = 0; index < owners.size(); index++) {
                result.add(new ExpandedProductionDefault(
                    owners.get(index), actual.inputType(), defaults.get(index)
                ));
            }
        }
        return result;
    }

    private static String effectiveDefaultContract(
        KeybindProductionSiteRow production,
        List<KeybindRow> migrationRows
    ) {
        return migrationRows.stream()
            .filter(row -> row.productionOwner().equals(production.sourcePath())
                && row.translationKey().equals(production.translationContract()))
            .map(KeybindRow::targetDefault)
            .findFirst()
            .orElse(production.normalizedDefaultContract());
    }

    private static List<String> expandedDefaultCodes(String defaultContract, int runtimeCardinality) {
        if (defaultContract.equals("F1..F9")) {
            assertEquals(9, runtimeCardinality,
                "quick-slot default expansion must retain its nine runtime bindings");
            return java.util.stream.IntStream.rangeClosed(1, 9)
                .mapToObj(index -> "F" + index)
                .toList();
        }
        assertEquals(1, runtimeCardinality,
            "non-quick-slot source sites must expand to one physical default");
        return List.of(defaultContract);
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
            if (!Files.exists(root)) {
                return;
            }
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
        assertTrue(declaration.contains("InputUtil.Type." + row.currentType()),
            "migration input type/default must be wired in the same KeyBinding declaration " + path + ": "
                + row.translationKey() + " -> " + row.currentType());
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

    private static boolean rowsUseValidPriorities(OpenPolicyRow row) {
        boolean requestTerminal = row.requestKind().equals("SYSTEM_TERMINAL");
        boolean currentTerminal = row.currentKind().equals("SYSTEM_TERMINAL");
        return requestTerminal == !row.requestPriority().equals("NONE")
            && currentTerminal == !row.currentPriority().equals("NONE");
    }

    @Test
    void keybindingRegistrationAuditRejectsLookalikesQualifiedAndUnsupportedOverloads() {
        assertDoesNotThrow(() -> auditKeyBindingSource("""
            import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
            class Probe {
                Object key = KeyBindingHelper.registerKeyBinding(new KeyBinding("key", type, code, "category"));
            }
            """));
        assertDoesNotThrow(() -> auditKeyBindingSource("""
            class Probe {
                Object key = net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper.registerKeyBinding(
                    new net.minecraft.client.option.KeyBinding("key", type, code, "category")
                );
            }
            """));
        assertDoesNotThrow(() -> auditKeyBindingSource("""
            import java.util.function.UnaryOperator;
            class Probe {
                void install(UnaryOperator<KeyBinding> registrar) {
                    registrar.apply(new KeyBinding("key", type, code, "category"));
                }
            }
            """));
        assertThrows(AssertionError.class, () -> auditKeyBindingSource("""
            class Probe {
                Object key = FakeKeyBindingHelper.registerKeyBinding(
                    new KeyBinding("key", type, code, "category")
                );
            }
            """));
        assertThrows(AssertionError.class, () -> auditKeyBindingSource("""
            import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
            class Probe {
                Object KeyBindingHelper;
                Object key = KeyBindingHelper.registerKeyBinding(
                    new KeyBinding("key", type, code, "category")
                );
            }
            """));
        assertThrows(AssertionError.class, () -> auditKeyBindingSource("""
            class Probe {
                Object registrar;
                void install() {
                    registrar.apply(new KeyBinding("key", type, code, "category"));
                }
            }
            """));
        assertThrows(AssertionError.class, () -> auditKeyBindingSource("""
            import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
            class Probe {
                Object key = KeyBindingHelper.registerKeyBinding(new KeyBinding("key", code, "category"));
            }
            """));
    }

    private static void auditKeyBindingSource(String source) {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        assertNotNull(compiler, "R7 keybinding scanner test requires a full Java 17 JDK");
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
            null, null, null, List.of("-proc:none"), null, List.of(sourceFile)
        );
        try {
            for (CompilationUnitTree unit : task.parse()) {
                new TreePathScanner<Void, Void>() {
                    @Override
                    public Void visitNewClass(NewClassTree tree, Void unused) {
                        if (isKeyBindingConstructor(tree)) {
                            assertEquals(4, tree.getArguments().size(),
                                "unsupported KeyBinding constructor overload: " + tree);
                            assertRegisteredKeyBinding(getCurrentPath(), Path.of("Probe.java"), unit);
                        }
                        return super.visitNewClass(tree, unused);
                    }
                }.scan(unit, null);
            }
        } catch (IOException exception) {
            throw new AssertionError("unable to parse keybinding scanner test source", exception);
        }
    }

    private static List<KeybindingSourceSite> productionKeybindingSourceSites() throws IOException {
        List<KeybindingSourceSite> result = new java.util.ArrayList<>();
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        assertNotNull(compiler, "R7 keybinding source scan requires a full Java 17 JDK");
        DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
        try (StandardJavaFileManager fileManager = compiler.getStandardFileManager(diagnostics, null, null)) {
            Iterable<? extends JavaFileObject> sources = fileManager.getJavaFileObjectsFromPaths(
                productionKeybindingJavaFiles()
            );
            JavacTask task = (JavacTask) compiler.getTask(
                null, fileManager, diagnostics, List.of("-proc:none"), null, sources
            );
            for (CompilationUnitTree unit : task.parse()) {
                Path path = Path.of(unit.getSourceFile().toUri());
                Map<String, ExpressionTree> constants = collectSourceConstants(unit);
                new TreePathScanner<Void, Void>() {
                    @Override
                    public Void visitNewClass(NewClassTree tree, Void unused) {
                        if (isKeyBindingConstructor(tree)) {
                            assertEquals(4, tree.getArguments().size(),
                                "every production KeyBinding constructor overload must be explicitly modeled in "
                                    + path + ": " + tree);
                            String sourceSite = enclosingAssignmentTarget(getCurrentPath());
                            assertRegisteredKeyBinding(getCurrentPath(), path, unit);
                            assertNotNull(sourceSite,
                                "every KeyBinding constructor needs a stable enclosing assignment target in " + path);
                            List<String> translations = resolveTranslationKeys(tree.getArguments().get(0), constants);
                            result.add(new KeybindingSourceSite(
                                CLIENT_ROOT.relativize(path).toString().replace('\\', '/'),
                                sourceSite,
                                translationContract(translations),
                                resolveInputType(tree.getArguments().get(1)),
                                resolveDefaultContract(tree.getArguments().get(2), constants),
                                resolveString(tree.getArguments().get(3), constants),
                                translations.size(),
                                translations
                            ));
                        }
                        return super.visitNewClass(tree, unused);
                    }
                }.scan(unit, null);
            }
            assertTrue(diagnostics.getDiagnostics().stream()
                    .noneMatch(diagnostic -> diagnostic.getKind() == Diagnostic.Kind.ERROR),
                "unable to parse production keybinding sources: " + diagnostics.getDiagnostics());
        }
        result.sort(java.util.Comparator
            .comparing(KeybindingSourceSite::sourcePath)
            .thenComparing(KeybindingSourceSite::sourceSite));
        return result;
    }

    private static List<Path> productionKeybindingJavaFiles() throws IOException {
        try (var files = Files.walk(CLIENT_ROOT)) {
            return files.filter(Files::isRegularFile)
                .filter(path -> path.getFileName().toString().endsWith(".java"))
                .sorted()
                .toList();
        }
    }

    private static Map<String, ExpressionTree> collectSourceConstants(CompilationUnitTree unit) {
        Map<String, ExpressionTree> constants = new java.util.HashMap<>();
        new TreePathScanner<Void, Void>() {
            @Override
            public Void visitVariable(VariableTree variable, Void unused) {
                if (variable.getInitializer() != null) {
                    constants.putIfAbsent(variable.getName().toString(), variable.getInitializer());
                }
                return super.visitVariable(variable, unused);
            }
        }.scan(unit, null);
        return constants;
    }

    private static List<String> resolveTranslationKeys(
        ExpressionTree expression,
        Map<String, ExpressionTree> constants
    ) {
        if (expression instanceof BinaryTree binary && binary.getKind() == Tree.Kind.PLUS) {
            String prefix = resolveString(binary.getLeftOperand(), constants);
            ExpressionTree right = binary.getRightOperand();
            if (right instanceof com.sun.source.tree.ParenthesizedTree parenthesized) {
                right = parenthesized.getExpression();
            }
            assertTrue(right instanceof BinaryTree indexExpression
                    && indexExpression.getKind() == Tree.Kind.PLUS
                    && indexExpression.getLeftOperand() instanceof IdentifierTree identifier
                    && identifier.getName().contentEquals("i")
                    && indexExpression.getRightOperand() instanceof LiteralTree literal
                    && Integer.valueOf(1).equals(literal.getValue()),
                "the only runtime-expanded key translation must be the quick-slot i + 1 expression");
            int count = quickSlotCount();
            return java.util.stream.IntStream.rangeClosed(1, count)
                .mapToObj(index -> prefix + index)
                .toList();
        }
        return List.of(resolveString(expression, constants));
    }

    private static int quickSlotCount() {
        Path config = CLIENT_ROOT.resolve("combat/QuickSlotConfig.java");
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        assertNotNull(compiler, "R7 quick-slot cardinality scan requires a full Java 17 JDK");
        DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
        try (StandardJavaFileManager fileManager = compiler.getStandardFileManager(diagnostics, null, null)) {
            JavacTask task = (JavacTask) compiler.getTask(
                null, fileManager, diagnostics, List.of("-proc:none"), null,
                fileManager.getJavaFileObjects(config.toFile())
            );
            for (CompilationUnitTree unit : task.parse()) {
                final int[] count = {-1};
                new TreePathScanner<Void, Void>() {
                    @Override
                    public Void visitVariable(VariableTree variable, Void unused) {
                        if (variable.getName().contentEquals("SLOT_COUNT")
                            && variable.getInitializer() instanceof LiteralTree literal
                            && literal.getValue() instanceof Integer value) {
                            count[0] = value;
                        }
                        return super.visitVariable(variable, unused);
                    }
                }.scan(unit, null);
                assertEquals(9, count[0],
                    "QuickSlotConfig.SLOT_COUNT is the production source of runtime keybind cardinality");
                return count[0];
            }
        } catch (IOException exception) {
            throw new AssertionError("unable to scan QuickSlotConfig.SLOT_COUNT", exception);
        }
        throw new AssertionError("missing QuickSlotConfig.SLOT_COUNT");
    }

    private static String resolveString(ExpressionTree expression, Map<String, ExpressionTree> constants) {
        if (expression instanceof LiteralTree literal && literal.getValue() instanceof String value) {
            return value;
        }
        if (expression instanceof IdentifierTree identifier) {
            ExpressionTree initializer = constants.get(identifier.getName().toString());
            assertNotNull(initializer, "unresolved source constant " + identifier.getName());
            return resolveString(initializer, constants);
        }
        throw new AssertionError("unsupported string contract expression: " + expression);
    }

    private static String resolveInputType(ExpressionTree expression) {
        if (expression instanceof MemberSelectTree select
            && select.getExpression() instanceof MemberSelectTree owner
            && owner.getIdentifier().contentEquals("Type")
            && owner.getExpression() instanceof IdentifierTree inputUtil
            && inputUtil.getName().contentEquals("InputUtil")) {
            return select.getIdentifier().toString();
        }
        throw new AssertionError("unsupported InputUtil.Type expression: " + expression);
    }

    private static String resolveDefaultContract(
        ExpressionTree expression,
        Map<String, ExpressionTree> constants
    ) {
        if (expression instanceof MethodInvocationTree invocation
            && invocation.getMethodSelect() instanceof MemberSelectTree select
            && select.getIdentifier().contentEquals("getCode")
            && select.getExpression() instanceof MemberSelectTree unknown
            && unknown.getIdentifier().contentEquals("UNKNOWN_KEY")) {
            return "UNKNOWN";
        }
        if (expression instanceof MemberSelectTree select
            && select.getIdentifier().contentEquals("GLFW_KEY_UNKNOWN")) {
            return "UNKNOWN";
        }
        if (expression instanceof BinaryTree binary && binary.getKind() == Tree.Kind.PLUS) {
            assertEquals("GLFW.GLFW_KEY_F1", compactExpression(binary.getLeftOperand()),
                "only quick slots may use an arithmetic default-key expression");
            assertEquals("i", compactExpression(binary.getRightOperand()),
                "quick-slot defaults must remain F1 + i");
            assertEquals(9, quickSlotCount());
            return "F1..F9";
        }
        if (expression instanceof IdentifierTree identifier) {
            ExpressionTree initializer = constants.get(identifier.getName().toString());
            assertNotNull(initializer, "unresolved default-key constant " + identifier.getName());
            return resolveDefaultContract(initializer, constants);
        }
        if (expression instanceof MemberSelectTree select) {
            String key = select.getIdentifier().toString();
            String prefix = "GLFW_KEY_";
            if (key.startsWith(prefix)) {
                return key.substring(prefix.length());
            }
        }
        throw new AssertionError("unsupported default-key contract expression: " + expression);
    }

    private static String translationContract(List<String> translations) {
        if (translations.size() == 1) {
            return translations.get(0);
        }
        assertEquals(java.util.stream.IntStream.rangeClosed(1, translations.size())
            .mapToObj(index -> "key.bong-client.quick_slot_" + index).toList(), translations,
            "the only runtime-expanded translations are quick slots 1 through SLOT_COUNT");
        return "key.bong-client.quick_slot_{1..9}";
    }

    private static boolean isKeyBindingConstructor(NewClassTree tree) {
        return Set.of("KeyBinding", "net.minecraft.client.option.KeyBinding")
            .contains(tree.getIdentifier().toString());
    }

    private static void assertRegisteredKeyBinding(
        TreePath path,
        Path sourcePath,
        CompilationUnitTree unit
    ) {
        TreePath parent = path.getParentPath();
        assertTrue(parent != null && parent.getLeaf() instanceof MethodInvocationTree,
            "every KeyBinding constructor must be passed directly to a registration call in " + sourcePath);
        MethodInvocationTree invocation = (MethodInvocationTree) parent.getLeaf();
        assertTrue(invocation.getArguments().contains(path.getLeaf()),
            "every KeyBinding constructor must be a direct registration argument in " + sourcePath);
        assertTrue(isRegistrationInvocation(invocation, parent, unit),
            "unregistered KeyBinding constructor in " + sourcePath + ": " + invocation.getMethodSelect());
    }

    private static boolean isRegistrationInvocation(
        MethodInvocationTree invocation,
        TreePath invocationPath,
        CompilationUnitTree unit
    ) {
        if (!(invocation.getMethodSelect() instanceof MemberSelectTree select)) {
            return false;
        }
        String method = select.getIdentifier().toString();
        if (method.equals("apply") && select.getExpression().toString().equals("registrar")) {
            return enclosingMethodHasRegistrarParameter(invocationPath);
        }
        if (!method.equals("registerKeyBinding")) {
            return false;
        }
        String receiver = select.getExpression().toString();
        return receiver.equals("net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper")
            || (receiver.equals("KeyBindingHelper")
                && !declaresIdentifier(unit, "KeyBindingHelper")
                && unit.getImports().stream().anyMatch(importTree ->
                !importTree.isStatic()
                    && importTree.getQualifiedIdentifier().toString().equals(
                        "net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper"
                    )));
    }

    private static boolean declaresIdentifier(CompilationUnitTree unit, String name) {
        boolean[] declared = {false};
        new TreePathScanner<Void, Void>() {
            @Override
            public Void visitVariable(VariableTree variable, Void unused) {
                declared[0] |= variable.getName().contentEquals(name);
                return super.visitVariable(variable, unused);
            }
        }.scan(unit, null);
        return declared[0];
    }

    private static boolean enclosingMethodHasRegistrarParameter(TreePath path) {
        for (TreePath cursor = path; cursor != null; cursor = cursor.getParentPath()) {
            if (cursor.getLeaf() instanceof com.sun.source.tree.MethodTree method) {
                return method.getParameters().stream().anyMatch(parameter ->
                    parameter.getName().contentEquals("registrar")
                        && parameter.getType().toString().replaceAll("\\s+", "")
                            .equals("UnaryOperator<KeyBinding>"));
            }
        }
        return false;
    }

    private static String enclosingAssignmentTarget(TreePath path) {
        for (TreePath cursor = path.getParentPath(); cursor != null; cursor = cursor.getParentPath()) {
            Tree leaf = cursor.getLeaf();
            if (leaf instanceof AssignmentTree assignment) {
                return compactExpression(assignment.getVariable());
            }
            if (leaf instanceof com.sun.source.tree.VariableTree variable) {
                return variable.getName().toString();
            }
        }
        return null;
    }

    private static String compactExpression(Tree tree) {
        return tree.toString().replaceAll("\\s+", " ").trim();
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

    private static List<String> expectedOpenPolicyFixtureLines() {
        return """
            social-expired-past\tSOCIAL_INVITE\tinvite-old\t999\tNONE\tfalse\tNONE\t\tNONE\tfalse\t1000\tEXPIRE\tA finite passive offer whose expiry is before now never opens.
            social-expired-boundary\tSOCIAL_INVITE\tinvite-boundary\t1000\tNONE\tfalse\tMODAL\tinvite-boundary\tNONE\tfalse\t1000\tEXPIRE\tExpiry is evaluated before identity matching when nowMs equals expiresAtMs, so an expired matching offer is never retained.
            social-open\tSOCIAL_INVITE\tinvite-live\t1001\tNONE\tfalse\tNONE\t\tNONE\tfalse\t1000\tOPEN\tA live passive offer opens only when gameplay is not in combat and no screen is active.
            social-matching\tSOCIAL_INVITE\tinvite-live\t1001\tNONE\tfalse\tMODAL\tinvite-live\tNONE\tfalse\t1000\tNOOP_MATCHING\tMatching derives from equal non-empty request/current identities.
            social-combat-first\tSOCIAL_INVITE\tinvite-live\t1001\tNONE\tfalse\tNONE\t\tNONE\ttrue\t1000\tDEFER_NOTIFY\tFirst blocked observation defers the authoritative offer and requests one caller-owned notification.
            social-combat-repeat\tSOCIAL_INVITE\tinvite-live\t1001\tNONE\ttrue\tNONE\t\tNONE\ttrue\t1000\tDEFER_SILENT\tThe same already-notified identity remains deferred without another notification.
            social-screen-first\tSOCIAL_INVITE\tinvite-live\t1001\tNONE\tfalse\tORDINARY\tinventory\tNONE\tfalse\t1000\tDEFER_NOTIFY\tAnother screen defers the offer until currentScreen becomes null and notifies once.
            social-screen-repeat\tSOCIAL_INVITE\tinvite-live\t1001\tNONE\ttrue\tORDINARY\tinventory\tNONE\tfalse\t1000\tDEFER_SILENT\tRepeated observation of the same blocked identity is silent.
            social-modal-first\tSOCIAL_INVITE\tinvite-live\t1001\tNONE\tfalse\tMODAL\ttrade-offer\tNONE\tfalse\t1000\tDEFER_NOTIFY\tA nonmatching modal defers the live social offer and notifies once.
            social-modal-repeat\tSOCIAL_INVITE\tinvite-live\t1001\tNONE\ttrue\tMODAL\ttrade-offer\tNONE\tfalse\t1000\tDEFER_SILENT\tRepeated observation behind the same nonmatching modal remains deferred without another notification.
            social-new-identity\tSOCIAL_INVITE\tinvite-new\t1001\tNONE\tfalse\tORDINARY\tinventory\tNONE\tfalse\t1000\tDEFER_NOTIFY\tA new caller-owned identity resets notification eligibility.
            social-terminal\tSOCIAL_INVITE\tinvite-live\t1001\tNONE\tfalse\tSYSTEM_TERMINAL\tdeath\tDEATH\tfalse\t1000\tDEFER_NOTIFY\tA passive social offer never displaces a system terminal.
            hotkey-open\tHOTKEY\tidentity-screen\t9223372036854775807\tNONE\tfalse\tNONE\t\tNONE\tfalse\t1000\tOPEN\tAn immediate user keypress may open when no screen blocks it.
            hotkey-matching\tHOTKEY\tidentity-screen\t9223372036854775807\tNONE\tfalse\tORDINARY\tidentity-screen\tNONE\tfalse\t1000\tNOOP_MATCHING\tA matching screen is not recreated.
            hotkey-ordinary\tHOTKEY\tidentity-screen\t9223372036854775807\tNONE\tfalse\tORDINARY\tinventory\tNONE\tfalse\t1000\tBLOCK_DROP\tAn ordinary nonmatching screen consumes the physical moment; the keypress is not queued.
            hotkey-modal\tHOTKEY\tidentity-screen\t9223372036854775807\tNONE\tfalse\tMODAL\ttrade-offer\tNONE\tfalse\t1000\tBLOCK_DROP\tPhysical keypresses are never queued for future replay behind a modal.
            hotkey-terminal\tHOTKEY\tidentity-screen\t9223372036854775807\tNONE\tfalse\tSYSTEM_TERMINAL\tdeath\tDEATH\tfalse\t1000\tBLOCK_DROP\tA hotkey never displaces or waits behind a system terminal.
            insight-expired\tINSIGHT\tinsight-old\t999\tNONE\tfalse\tNONE\t\tNONE\tfalse\t1000\tEXPIRE\tInsightOfferScreenBootstrap settles the expired offer instance identified by offer_id before screen creation and the policy never opens a screen.
            insight-open\tINSIGHT\tinsight-live\t1001\tNONE\tfalse\tNONE\t\tNONE\tfalse\t1000\tOPEN\tA live insight opens when no UI is active.
            insight-preempt\tINSIGHT\tinsight-live\t1001\tNONE\tfalse\tORDINARY\tinventory\tNONE\tfalse\t1000\tPREEMPT\tInsight may replace ordinary non-modal UI through transition arbitration.
            insight-matching\tINSIGHT\tinsight-live\t1001\tNONE\tfalse\tMODAL\tinsight-live\tNONE\tfalse\t1000\tNOOP_MATCHING\tThe same insight identity is not reopened.
            insight-modal-first\tINSIGHT\tinsight-live\t1001\tNONE\tfalse\tMODAL\ttrade-offer\tNONE\tfalse\t1000\tDEFER_NOTIFY\tInsight waits behind an equal or higher modal and notifies once.
            insight-modal-repeat\tINSIGHT\tinsight-live\t1001\tNONE\ttrue\tMODAL\ttrade-offer\tNONE\tfalse\t1000\tDEFER_SILENT\tRepeated blocked insight observation is silent.
            insight-terminal\tINSIGHT\tinsight-live\t1001\tNONE\tfalse\tSYSTEM_TERMINAL\tdeath\tDEATH\tfalse\t1000\tDEFER_NOTIFY\tInsight never displaces death or termination UI.
            death-open\tSYSTEM_TERMINAL\tdeath-1\t9223372036854775807\tDEATH\tfalse\tNONE\t\tNONE\tfalse\t1000\tOPEN\tA death terminal opens when no screen is active.
            death-matching\tSYSTEM_TERMINAL\tdeath-1\t9223372036854775807\tDEATH\tfalse\tSYSTEM_TERMINAL\tdeath-1\tDEATH\tfalse\t1000\tNOOP_MATCHING\tThe same terminal identity is not recreated.
            death-preempt-ordinary\tSYSTEM_TERMINAL\tdeath-1\t9223372036854775807\tDEATH\tfalse\tORDINARY\tinventory\tNONE\tfalse\t1000\tPREEMPT\tDeath may displace lower-priority ordinary UI.
            death-preempt-modal\tSYSTEM_TERMINAL\tdeath-1\t9223372036854775807\tDEATH\tfalse\tMODAL\ttrade-offer\tNONE\tfalse\t1000\tPREEMPT\tDeath may displace lower-priority modal UI.
            terminate-preempt-death\tSYSTEM_TERMINAL\tterminate-1\t9223372036854775807\tTERMINATE\tfalse\tSYSTEM_TERMINAL\tdeath-1\tDEATH\tfalse\t1000\tPREEMPT\tTerminate explicitly outranks Death.
            death-blocked-terminate\tSYSTEM_TERMINAL\tdeath-1\t9223372036854775807\tDEATH\tfalse\tSYSTEM_TERMINAL\tterminate-1\tTERMINATE\tfalse\t1000\tBLOCK_DROP\tDeath cannot preempt a visible Terminate terminal.
            death-blocked-peer\tSYSTEM_TERMINAL\tdeath-2\t9223372036854775807\tDEATH\tfalse\tSYSTEM_TERMINAL\tdeath-1\tDEATH\tfalse\t1000\tBLOCK_DROP\tA nonmatching equal-priority terminal is not replaced.
            terminate-blocked-peer\tSYSTEM_TERMINAL\tterminate-2\t9223372036854775807\tTERMINATE\tfalse\tSYSTEM_TERMINAL\tterminate-1\tTERMINATE\tfalse\t1000\tBLOCK_DROP\tA nonmatching Terminate terminal cannot replace an equal-priority peer.
            """.strip().lines().toList();
    }

    private static List<InsightSettlementRow> expectedInsightSettlementRows() {
        return """
            ACCEPT	InsightDecision.chosen(triggerId, choiceId)	InsightOfferScreen	claim only the exact current offerId; triggerId remains decision context and may recur	Atomically commit ACCEPT for the current offerId before sending; then send; then close this screen if still current.	A send failure is primary and closing this screen is still attempted.	A close failure leaves this offerId terminal; later paths for the same instance remain NOOP.	If send and close both fail, retain send as primary and suppress close in execution order.	Emit at most one CHOSEN decision for this offer instance; terminal state is discarded once no current or pending lifecycle can race with it.
            DECLINE	InsightDecision.declined(triggerId)	InsightOfferScreen	claim only the exact current offerId; a later offer may reuse triggerId	Atomically commit DECLINE for the current offerId before sending; then send; then close this screen if still current.	A send failure is primary and closing this screen is still attempted.	A close failure leaves this offerId terminal; later paths for the same instance remain NOOP.	If send and close both fail, retain send as primary and suppress close in execution order.	Emit at most one DECLINED decision for this offer instance without poisoning a later offer that reuses triggerId.
            TIMEOUT	InsightDecision.timedOut(triggerId)	InsightOfferScreenBootstrap + InsightOfferScreen	pre-open bootstrap and open screen compete for one exact offerId claim	Atomically commit TIMEOUT for offerId before sending; pre-open expiry creates no screen, while an open screen then closes if still current.	A send failure is primary; the applicable close or transition is still attempted.	A close or removal failure leaves this offerId terminal and cannot authorize another send.	If send and transition both fail, retain send as primary and suppress transition in execution order.	Emit one TIMED_OUT decision for the offer instance; stale timeout of offer A cannot clear current or pending offer B.
            ESC	InsightDecision.declined(triggerId)	InsightOfferScreen.close	claim the exact offerId owned by the closing screen	Atomically commit ESC for offerId before sending; then send; then close this screen if still current.	A send failure is primary and closing this screen is still attempted.	Close or removal failure leaves this offerId terminal; onRemoved remains NOOP for settlement.	Retain the first failure and suppress later close/removal failures in execution order.	Emit at most one DECLINED decision for this offer instance.
            REPLACED_BY_DIFFERENT_OFFER	InsightDecision.declined(triggerId)	InsightOfferStore + InsightOfferScreenBootstrap + ScreenTransitionController	compare outgoing and incoming offerId; settle an outgoing current or pending offer even when no InsightOfferScreen is current	Publish the incoming offer into a bounded pending slot; commit and send the outgoing offerId before attempting the screen switch; promote pending to current only after installation succeeds.	An outgoing send failure is primary and installation is still attempted; its offerId remains terminal.	Installation failure keeps the incoming offer non-authoritative in the pending slot for retry; it cannot strand an authoritative snapshot without a screen.	If send and installation both fail, retain send as primary and suppress installation in execution order.	Outgoing emits at most one DECLINED decision; incoming is either installed as current or remains bounded pending and reachable for retry.
            ANIMATED_OPEN_CANCELLED	InsightDecision.declined(triggerId)	InsightOfferScreen + ScreenTransitionController.PendingOpenCancellationHandler	claim the exact offerId owned by activeTransition.handle().newScreen()	Atomically commit ANIMATED_OPEN_CANCELLED for offerId before sending the decline; then clear only matching current/pending ownership.	A send failure remains observable after the offerId is committed terminal.	Cancellation cannot leave the offer current or pending without a visible or retryable screen.	A later removal callback is NOOP for settlement and any later lifecycle failure is suppressed after the send failure.	Cancelling the 250 ms animated open emits at most one decline and leaves no invisible unsettled offer.
            REMOVED_EXCEPTIONALLY	InsightDecision.declined(triggerId)	InsightOfferScreen.onRemoved	claim the screen's exact offerId only when no earlier cause won	Atomically commit REMOVED_EXCEPTIONALLY for offerId before sending; then continue all base removal stages.	A send failure is primary and later removal stages are still attempted.	A later removal-stage failure cannot replace the committed cause.	BongScreenBase retains the first failure and suppresses later lifecycle failures in execution order.	Exceptional removal emits at most one decline and cannot clear a different current or pending offer.
            DUPLICATE_TERMINAL	NOOP	InsightOfferStore	an already committed offerId is immutable only while its current/pending lifecycle can still race	Observe the bounded in-flight terminal claim and return before send or transition.	No decision send is attempted.	No screen transition is attempted and no different offerId is mutated.	No new exception is produced.	No duplicate decision or transition occurs; historical offer IDs are not retained after their lifecycle becomes unreachable.
            """.strip().lines()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new InsightSettlementRow(
                columns[0], columns[1], columns[2], columns[3], columns[4],
                columns[5], columns[6], columns[7], columns[8]
            ))
            .toList();
    }

    private static List<FoundationRow> expectedFoundationRows() {
        return List.of(
            new FoundationRow("BongScreenBase", "type", "public abstract class BongScreenBase<R extends ParentComponent> extends BaseOwoScreen<R>", "R7", "Root type remains generic; subclasses keep ownership of the owo adapter."),
            new FoundationRow("BongScreenBase", "constructor-empty", "protected BongScreenBase()", "R7", "Supports existing screens that use the owo default title path."),
            new FoundationRow("BongScreenBase", "constructor-title", "protected BongScreenBase(Text title)", "R7", "Rejects a null title and delegates the title to BaseOwoScreen."),
            new FoundationRow("BongScreenBase", "adapter", "protected abstract @NotNull OwoUIAdapter<R> createAdapter()", "R7", "The base never hard-codes OwoUIAdapter.create or a vertical-flow root factory."),
            new FoundationRow("BongScreenBase", "build", "protected abstract void build(R rootComponent)", "R7", "The base delegates layout construction to the concrete screen."),
            new FoundationRow("BongScreenBase", "cleanup", "protected final void registerCleanup(Runnable cleanup)", "R7", "Screen-local cleanup runs exactly once in LIFO order; removal still reaches every lifecycle stage; first failure is primary and later failures are suppressed in execution order; repeated removal is a no-op and this is not Store lifecycle clearing."),
            new FoundationRow("BongScreenBase", "refresh", "protected final void runWhileOpen(Runnable task)", "R7", "A queued refresh is discarded after removal and cannot touch a detached screen."),
            new FoundationRow("BongScreenBase", "tick", "public final void tick()", "R7", "Tick dispatch reaches the protected hook only while the screen is open."),
            new FoundationRow("BongScreenBase", "tick-hook", "protected void onScreenTick()", "R7", "Subclasses extend tick behavior without bypassing the open-state guard."),
            new FoundationRow("BongScreenBase", "removed", "public final void removed()", "R7", "Removal marks closed first; business hook, LIFO cleanup, and super.removed remain reachable after failures; the first failure is thrown with later failures suppressed; repeated removal is a no-op."),
            new FoundationRow("BongScreenBase", "removed-hook", "protected void onRemoved()", "R7", "Business terminal effects run once without allowing cleanup bypass."),
            new FoundationRow("DiffListWidget", "type", "public final class DiffListWidget<T, K, C extends Component>", "R7", "The generic list owns ordered-key diffing without imposing an owo scroll-offset API."),
            new FoundationRow("DiffListWidget", "constructor", "public DiffListWidget(FlowLayout rows, Function<? super T, ? extends K> keyOf, Function<? super T, ? extends C> createRow, BiConsumer<? super C, ? super T> patchRow)", "R7", "A final widget receives key extraction and row lifecycle functions by constructor injection."),
            new FoundationRow("DiffListWidget", "update", "public UpdateResult update(List<? extends T> items)", "R7", "Null list/item/key and duplicate keys fail before mutation; equal ordered keys patch mounted rows; patch failure propagates unchanged, preserves the previous committed key sequence, and the next update retries the full list; structural key changes create every replacement row before mutating mounted children, so createRow failure preserves the prior mounted rows and committed keys, and a successful rebuild swaps the complete row set atomically."),
            new FoundationRow("DiffListWidget", "rendered-keys", "public List<K> renderedKeys()", "R7", "Inspection returns the immutable current ordered key sequence."),
            new FoundationRow("DiffListWidget", "row-lookup", "public Optional<C> rowForKey(K key)", "R7", "Inspection exposes mounted identity without leaking mutable ownership."),
            new FoundationRow("DiffListWidget", "result", "public enum UpdateResult { REBUILT, PATCHED }", "R7", "The caller can distinguish structural rebuild from identity-preserving patch."),
            new FoundationRow("BongKeybindRegistry", "type", "public final class BongKeybindRegistry", "R7", "Registrations are explicit and inspectable; no reflection or annotation discovery."),
            new FoundationRow("BongKeybindRegistry", "global", "public static BongKeybindRegistry global()", "R7", "Production has one registry instance so bootstrap-local registries cannot bypass conflict detection."),
            new FoundationRow("BongKeybindRegistry", "constructor", "BongKeybindRegistry(UnaryOperator<KeyBinding> registrar, List<ReservedDefault> reservedDefaults, Set<ConflictExemption> exemptions)", "R7", "The package seam injects Fabric registration plus explicit vanilla reservations and exact exemptions."),
            new FoundationRow("BongKeybindRegistry", "register", "public KeyBinding register(BindingSpec spec)", "R7", "Duplicate owner identities and translation keys always fail; UNKNOWN defaults do not collide physically."),
            new FoundationRow("BongKeybindRegistry", "registrations", "public List<Registration> registrations()", "R7", "Inspection is immutable and preserves registration order."),
            new FoundationRow("BongKeybindRegistry", "binding-owner", "public record BindingOwner(String id)", "R7", "A non-blank globally unique owner id identifies one logical binding independent of its source file or translation text."),
            new FoundationRow("BongKeybindRegistry", "binding-spec", "public record BindingSpec(BindingOwner owner, String translationKey, InputUtil.Type type, int defaultCode, String category)", "R7", "Every binding carries its explicit owner identity; physical identity is the exact InputUtil.Type plus default code pair."),
            new FoundationRow("BongKeybindRegistry", "registration", "public record Registration(BindingOwner owner, BindingSpec spec, KeyBinding binding)", "R7", "A successful immutable registration repeats the exact owner identity next to its spec and Fabric binding for unambiguous inspection."),
            new FoundationRow("BongKeybindRegistry", "physical-default", "public record PhysicalDefault(InputUtil.Type type, int code)", "R7", "Physical collision identity does not collapse different InputUtil types sharing one numeric code."),
            new FoundationRow("BongKeybindRegistry", "reserved-default", "public record ReservedDefault(BindingOwner owner, PhysicalDefault key)", "R7", "Vanilla-reserved defaults use the same explicit owner identity namespace as Bong bindings."),
            new FoundationRow("BongKeybindRegistry", "conflict-exemption", "public record ConflictExemption(BindingOwner firstOwner, BindingOwner secondOwner, PhysicalDefault key, String reason)", "R7", "An exemption applies only to an exact canonical owner-id pair, exact physical key, and non-empty reason."),
            new FoundationRow("ClientThreadMarshal", "run-production", "public static boolean run(Runnable task)", "R7", "Null MinecraftClient fails closed; accepted inline or queued work returns true and runs the task once."),
            new FoundationRow("ClientThreadMarshal", "run-seam", "static boolean run(Supplier<Boolean> onClientThread, Runnable task, Consumer<Runnable> clientExecutor)", "R7", "True runs once inline; false enqueues once; null predicate result performs neither and returns false; task failures propagate unchanged."),
            new FoundationRow("ScreenOpenPolicy", "decide", "public static Decision decide(Request request, Current current, long nowMs)", "R7", "The policy is pure; matching derives from request/current identity, expiry is nowMs greater than or equal to expiresAtMs, notification state selects notify versus silent defer, and it never calls MinecraftClient.setScreen or owns a second pending-offer store."),
            new FoundationRow("ScreenOpenPolicy", "request-kind", "public enum RequestKind { SOCIAL_INVITE, HOTKEY, INSIGHT, SYSTEM_TERMINAL }", "R7", "Request kinds have distinct defer, drop, and preemption semantics."),
            new FoundationRow("ScreenOpenPolicy", "current-kind", "public enum CurrentKind { NONE, ORDINARY, MODAL, SYSTEM_TERMINAL }", "R7", "Current UI priority is explicit and independent of concrete Screen classes."),
            new FoundationRow("ScreenOpenPolicy", "terminal-priority", "public enum TerminalPriority { NONE, DEATH, TERMINATE }", "R7", "TERMINATE outranks DEATH by explicit comparison rather than enum ordinal."),
            new FoundationRow("ScreenOpenPolicy", "decision", "public enum Decision { OPEN, PREEMPT, NOOP_MATCHING, DEFER_NOTIFY, DEFER_SILENT, BLOCK_DROP, EXPIRE }", "R7", "System-terminal and higher-priority modal screens preempt lower UI; passive social offers defer until safe or expiry; repeated blocked identities defer silently; physical hotkeys drop."),
            new FoundationRow("ScreenOpenPolicy", "request", "public record Request(RequestKind kind, String identity, long expiresAtMs, TerminalPriority terminalPriority, boolean alreadyNotified)", "R7", "Matching derives from identity and finite TTL expires at nowMs greater than or equal to expiresAtMs; notification state belongs to the caller/domain owner."),
            new FoundationRow("ScreenOpenPolicy", "current", "public record Current(CurrentKind kind, String identity, TerminalPriority terminalPriority, boolean combatActive)", "R7", "NONE has empty identity; only system terminal state may carry DEATH or TERMINATE priority.")
        );
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
                columns[0], columns[1], columns[2], columns[3], columns[4], columns[5],
                columns[6], columns[7], columns[8]
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
                Long.parseLong(columns[3]),
                columns[4],
                Boolean.parseBoolean(columns[5]),
                columns[6],
                columns[7],
                columns[8],
                Boolean.parseBoolean(columns[9]),
                Long.parseLong(columns[10]),
                columns[11],
                columns[12]
            ))
            .toList();
    }

    private static List<ReservedDefaultRow> reservedDefaultRows() {
        return resourceLines("/bong/ui/r7-keybind-reserved-defaults.tsv").stream()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new ReservedDefaultRow(columns[0], columns[1], columns[2], columns[3]))
            .toList();
    }

    private static List<ConflictExemptionRow> conflictExemptionRows() {
        return resourceLines("/bong/ui/r7-keybind-conflict-exemptions.tsv").stream()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new ConflictExemptionRow(
                columns[0], columns[1], columns[2], columns[3], columns[4]
            ))
            .toList();
    }

    private static List<KeybindProductionSiteRow> keybindProductionSiteRows() {
        return resourceLines("/bong/ui/r7-keybind-production-sites.tsv").stream()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new KeybindProductionSiteRow(
                columns[0], columns[1], columns[2], columns[3], columns[4],
                columns[5], columns[6], columns[7]
            ))
            .toList();
    }

    private static List<InsightSettlementRow> insightSettlementRows() {
        return resourceLines("/bong/ui/r7-insight-settlement.tsv").stream()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new InsightSettlementRow(
                columns[0], columns[1], columns[2], columns[3], columns[4],
                columns[5], columns[6], columns[7], columns[8]
            ))
            .toList();
    }
    private static List<String> resourceLines(String name) {
        try {
            var resource = R7FoundationContractTest.class.getResource(name);
            assertNotNull(resource, "missing R7 fixture " + name);
            return Files.readAllLines(Path.of(resource.toURI())).stream()
                .filter(R7SourceScan::isFixtureDataLine)
                .map(line -> line.replaceFirst("^\\d+\\t", ""))
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
        String currentType,
        String currentDefault,
        String targetType,
        String targetDefault,
        String conflict,
        String productionOwner,
        String resolution
    ) {
    }

    private record PhysicalDefault(String type, String code) {
    }

    private record ExpandedProductionDefault(String ownerId, String inputType, String defaultCode) {
    }

    private record DefaultCollision(String productionOwnerId, String reservedOwnerId, String inputType, String code)
        implements Comparable<DefaultCollision> {
        @Override
        public int compareTo(DefaultCollision other) {
            int ownerOrder = productionOwnerId.compareTo(other.productionOwnerId);
            if (ownerOrder != 0) {
                return ownerOrder;
            }
            int reservedOrder = reservedOwnerId.compareTo(other.reservedOwnerId);
            if (reservedOrder != 0) {
                return reservedOrder;
            }
            int typeOrder = inputType.compareTo(other.inputType);
            return typeOrder != 0 ? typeOrder : code.compareTo(other.code);
        }
    }

    private record ReservedDefaultRow(String owner, String inputType, String code, String reason) {
    }

    private record ConflictExemptionRow(
        String firstOwnerId,
        String secondOwnerId,
        String inputType,
        String code,
        String reason
    ) {
    }

    private record KeybindProductionSiteRow(
        String ownerId,
        String sourcePath,
        String sourceSite,
        String translationContract,
        String inputType,
        String defaultContract,
        String categoryContract,
        String runtimeCardinality
    ) {
        String fixtureLine() {
            return String.join("\t", ownerId, sourcePath, sourceSite, translationContract, inputType,
                defaultContract, categoryContract, runtimeCardinality);
        }

        String normalizedDefaultContract() {
            return defaultContract.startsWith("DEFAULT_KEY=")
                ? defaultContract.substring("DEFAULT_KEY=".length())
                : defaultContract;
        }

        int runtimeCardinalityCount() {
            return runtimeCardinality.startsWith("9 ") ? 9 : Integer.parseInt(runtimeCardinality);
        }

        String translationSourceContract() {
            return switch (ownerId) {
                case "combat.quick_slot" -> "\"key.bong-client.quick_slot_\" + (i + 1)";
                case "combat.jiemai_react", "combat.spell_volume_hold",
                    "combat.event_stream_toggle", "combat.shield_hold" -> "\"" + translationContract + "\"";
                default -> switch (sourceSite) {
                    case "autoHarvestKey" -> "AUTO_KEY_TRANSLATION";
                    case "giveDanKey" -> "KEY_GIVE_DAN";
                    case "refuseKey" -> "KEY_REFUSE";
                    case "delayKey" -> "KEY_DELAY";
                    case "interactKey" -> "INTERACT_KEY_TRANSLATION";
                    case "dashKey" -> "DASH_KEY_TRANSLATION";
                    case "senseKey" -> "SENSE_KEY_TRANSLATION";
                    case "markKey" -> "MARK_KEY_TRANSLATION";
                    default -> switch (ownerId) {
                        case "combat.juice_multiplier_cycle", "npc.interaction_log" -> "KEY_TRANSLATION";
                        case "hud.immersive_toggle" -> "TOGGLE_KEY";
                        case "tsy.extract_start" -> "EXTRACT_KEY_TRANSLATION";
                        case "tsy.extract_cancel", "tsy.search_cancel" -> "CANCEL_KEY_TRANSLATION";
                        default -> "OPEN_KEY_TRANSLATION";
                    };
                };
            };
        }

        String defaultSourceContract() {
            if (defaultContract.startsWith("DEFAULT_KEY=")) {
                return "DEFAULT_KEY";
            }
            return switch (defaultContract) {
                case "UNKNOWN" -> ownerId.startsWith("dying_elder.")
                    ? "InputUtil.UNKNOWN_KEY.getCode()"
                    : "GLFW.GLFW_KEY_UNKNOWN";
                case "F1..F9" -> "GLFW.GLFW_KEY_F1 + i";
                case "G" -> "DEFAULT_KEY_CODE";
                default -> "GLFW.GLFW_KEY_" + defaultContract;
            };
        }

        String categorySourceContract() {
            return "CATEGORY";
        }

        List<String> expandedTranslationKeys() {
            if (!translationContract.equals("key.bong-client.quick_slot_{1..9}")) {
                return List.of(translationContract);
            }
            return java.util.stream.IntStream.rangeClosed(1, 9)
                .mapToObj(index -> "key.bong-client.quick_slot_" + index)
                .toList();
        }
    }

    private record SourceSiteIdentity(String sourcePath, String sourceSite)
        implements Comparable<SourceSiteIdentity> {
        @Override
        public int compareTo(SourceSiteIdentity other) {
            int pathOrder = sourcePath.compareTo(other.sourcePath);
            return pathOrder != 0 ? pathOrder : sourceSite.compareTo(other.sourceSite);
        }
    }

    private record KeybindingSourceSite(
        String sourcePath,
        String sourceSite,
        String translationContract,
        String inputType,
        String defaultContract,
        String categoryContract,
        int runtimeCardinality,
        List<String> expandedTranslationKeys
    ) {
    }

    private record InsightSettlementRow(
        String terminalCause,
        String settlement,
        String owner,
        String identityRule,
        String commitOrder,
        String sendFailure,
        String transitionFailure,
        String exceptionRule,
        String observableEffect
    ) {
    }

    private record OpenPolicyRow(
        String scenario,
        String requestKind,
        String requestIdentity,
        long expiresAtMs,
        String requestPriority,
        boolean alreadyNotified,
        String currentKind,
        String currentIdentity,
        String currentPriority,
        boolean combatActive,
        long nowMs,
        String decision,
        String rationale
    ) {
        boolean matching() {
            return !requestIdentity.isBlank() && requestIdentity.equals(currentIdentity);
        }

        boolean expired() {
            return expiresAtMs != Long.MAX_VALUE && nowMs >= expiresAtMs;
        }
    }
}
