package com.bong.client.ui;

import com.sun.source.tree.AssignmentTree;
import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.NewClassTree;
import com.sun.source.tree.Tree;
import com.sun.source.util.JavacTask;
import com.sun.source.util.TreePath;
import com.sun.source.util.TreePathScanner;
import org.junit.jupiter.api.Test;

import javax.tools.Diagnostic;
import javax.tools.DiagnosticCollector;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
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
            "all 30 raw policy vectors and every input/output field must be explicitly re-decided");
        List<OpenPolicyRow> rows = openPolicyRows();
        assertEquals(30, rows.size(), "ScreenOpenPolicy P0 decision vectors changed");
        assertEquals(30, rows.stream().map(OpenPolicyRow::scenario)
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
        assertEquals(expectedInsightSettlementFixtureLines(), lines,
            "every settlement, owner, identity guard, commit order, failure rule, and observable effect must be exact-pinned");
        List<InsightSettlementRow> rows = insightSettlementRows();
        assertEquals(Set.of(
            "ACCEPT", "DECLINE", "TIMEOUT", "ESC", "REPLACED_BY_DIFFERENT_OFFER",
            "REMOVED_EXCEPTIONALLY", "DUPLICATE_TERMINAL"
        ), rows.stream().map(InsightSettlementRow::terminalCause)
            .collect(java.util.stream.Collectors.toSet()),
            "every insight terminal cause must have one exactly-once settlement contract");
        assertEquals(7, rows.size(), "insight terminal causes must be unique");
        assertTrue(rows.stream().allMatch(row -> row.identityRule().contains("triggerId")),
            "settlement must be identity guarded by the offer triggerId");
        assertTrue(rows.stream()
            .filter(row -> !row.terminalCause().equals("DUPLICATE_TERMINAL"))
            .allMatch(row -> row.commitOrder().contains("commit")
                && row.commitOrder().contains("before sending")),
            "the winning triggerId and cause must commit before any fallible decision send");
        assertTrue(rows.stream()
            .filter(row -> !row.terminalCause().equals("DUPLICATE_TERMINAL"))
            .allMatch(row -> row.sendFailure().contains("still attempted")
                || row.terminalCause().equals("REMOVED_EXCEPTIONALLY")),
            "a send failure cannot skip the following transition or removal lifecycle stage");
        assertTrue(rows.stream().allMatch(row -> row.transitionFailure().contains("committed winner")
            || row.transitionFailure().contains("winner committed")),
            "transition failures and duplicate paths must preserve the first committed winner");
        assertEquals("NOOP", rows.stream()
            .filter(row -> row.terminalCause().equals("DUPLICATE_TERMINAL"))
            .findFirst().orElseThrow().settlement(),
            "a second terminal path cannot emit another InsightDecision");
        assertTrue(rows.stream()
            .filter(row -> row.terminalCause().equals("REPLACED_BY_DIFFERENT_OFFER"))
            .findFirst().orElseThrow().owner().contains("CurrentScreenCancellationHandler"),
            "replacement settlement must compose with ScreenTransitionController cancellation");
    }

    @Test
    void keybindManifestsPinReservedDefaultsExemptionsAndEveryProductionSite() throws IOException {
        assertEquals(List.of(
            new ReservedDefaultRow("vanilla.chat", "KEYSYM", "T",
                "Minecraft chat default is reserved before Bong registrations."),
            new ReservedDefaultRow("vanilla.advancements", "KEYSYM", "L",
                "Minecraft advancements default is reserved before Bong registrations.")
        ), reservedDefaultRows(), "vanilla reservation manifest drifted");
        assertTrue(conflictExemptionRows().isEmpty(),
            "R7 P0 authorizes no physical-default conflict exemption");

        List<KeybindProductionSiteRow> expected = keybindProductionSiteRows();
        List<R7SourceScan.TokenOccurrence> actualSites = productionKeybindingConstructorSites();
        assertEquals(26, actualSites.size(), "production KeyBinding constructor-site count changed");
        assertEquals(26, expected.size(), "each constructor source site must have one exact binding contract");
        assertEquals(26, expected.stream().map(KeybindProductionSiteRow::ownerId)
            .collect(java.util.stream.Collectors.toSet()).size(),
            "every logical binding needs one globally unique BindingOwner id");
        Map<String, Long> actualByPath = new TreeMap<>();
        for (R7SourceScan.TokenOccurrence occurrence : actualSites) {
            actualByPath.merge(occurrence.path(), 1L, Long::sum);
        }
        Map<String, Long> expectedByPath = new TreeMap<>();
        for (KeybindProductionSiteRow row : expected) {
            expectedByPath.merge(row.sourcePath(), 1L, Long::sum);
        }
        assertEquals(actualByPath, expectedByPath,
            "the exact contract inventory must cover every production constructor source site");
        assertEquals(34, expected.stream().mapToInt(KeybindProductionSiteRow::runtimeCardinalityCount).sum(),
            "26 source constructors expand to exactly 34 runtime bindings (nine quick slots)");
        for (KeybindProductionSiteRow row : expected) {
            String source = R7SourceScan.codeOnly(R7SourceScan.read(CLIENT_ROOT.resolve(row.sourcePath())));
            assertTrue(source.contains(row.routeAnchor()),
                "behavior route anchor drifted for " + row.ownerId());
            KeybindingSourceContract actual = keybindingSourceContract(row);
            assertEquals(row.sourceSite(), actual.sourceSite(),
                "stable assignment target drifted for " + row.ownerId());
            assertEquals(row.translationSourceContract(), actual.translationArgument(),
                "translation constructor argument drifted for " + row.ownerId());
            assertEquals("InputUtil.Type." + row.inputType(), actual.inputTypeArgument(),
                "InputUtil.Type constructor argument drifted for " + row.ownerId());
            assertEquals(row.defaultSourceContract(), actual.defaultArgument(),
                "default-code constructor argument drifted for " + row.ownerId());
            assertEquals(row.categorySourceContract(), actual.categoryArgument(),
                "category constructor argument drifted for " + row.ownerId());
            assertFalse(row.consumerRoute().isBlank(),
                "each binding must freeze its behavior-critical consumer route: " + row.ownerId());
        }
        KeybindProductionSiteRow quickSlots = expected.stream()
            .filter(row -> row.ownerId().equals("combat.quick_slot"))
            .findFirst().orElseThrow();
        String combatSource = R7SourceScan.codeOnly(R7SourceScan.read(CLIENT_ROOT.resolve(quickSlots.sourcePath())))
            .replaceAll("\\s+", " ");
        assertTrue(combatSource.contains("i < QuickSlotConfig.SLOT_COUNT")
            && combatSource.contains("QUICK_SLOT_KEYS[i]")
            && combatSource.contains("while (QUICK_SLOT_KEYS[i].wasPressed())")
            && combatSource.contains("quickSlotHandler.accept(i)"),
            "quick-slot one-source/nine-runtime expansion and ordered full-drain route must stay frozen");
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

    private static List<R7SourceScan.TokenOccurrence> productionKeybindingConstructorSites() throws IOException {
        return R7SourceScan.tokenOccurrences(CLIENT_ROOT, "new KeyBinding(").stream()
            .filter(R7SourceScan.TokenOccurrence::code)
            .toList();
    }

    private static KeybindingSourceContract keybindingSourceContract(KeybindProductionSiteRow expected) {
        Path path = CLIENT_ROOT.resolve(expected.sourcePath());
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        assertNotNull(compiler, "R7 keybinding contract scan requires a full Java 17 JDK");
        DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
        try (StandardJavaFileManager fileManager = compiler.getStandardFileManager(diagnostics, null, null)) {
            Iterable<? extends JavaFileObject> sources = fileManager.getJavaFileObjects(path.toFile());
            JavacTask task = (JavacTask) compiler.getTask(
                null, fileManager, diagnostics, List.of("-proc:none"), null, sources
            );
            List<KeybindingSourceContract> candidates = new java.util.ArrayList<>();
            for (CompilationUnitTree unit : task.parse()) {
                new TreePathScanner<Void, Void>() {
                    @Override
                    public Void visitNewClass(NewClassTree tree, Void unused) {
                        if (tree.getIdentifier().toString().equals("KeyBinding") && tree.getArguments().size() == 4) {
                            String site = enclosingAssignmentTarget(getCurrentPath());
                            if (site != null && site.equals(expected.sourceSite())) {
                                candidates.add(new KeybindingSourceContract(
                                    site,
                                    compactExpression(tree.getArguments().get(0)),
                                    compactExpression(tree.getArguments().get(1)),
                                    compactExpression(tree.getArguments().get(2)),
                                    compactExpression(tree.getArguments().get(3))
                                ));
                            }
                        }
                        return super.visitNewClass(tree, unused);
                    }
                }.scan(unit, null);
            }
            assertTrue(diagnostics.getDiagnostics().stream()
                    .noneMatch(diagnostic -> diagnostic.getKind() == Diagnostic.Kind.ERROR),
                "unable to parse production keybinding source " + path + ": " + diagnostics.getDiagnostics());
            assertEquals(1, candidates.size(),
                "source-site identity must resolve exactly one KeyBinding constructor: "
                    + expected.ownerId() + " in " + path);
            return candidates.get(0);
        } catch (IOException exception) {
            throw new AssertionError("unable to scan production keybinding source " + path, exception);
        }
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

    private static List<String> expectedInsightSettlementFixtureLines() {
        return """
            ACCEPT\tInsightDecision.chosen(triggerId, choiceId)\tInsightOfferScreen\tsettle only when current offer triggerId matches\tAtomically commit settled triggerId and ACCEPT as the immutable winning cause before sending the decision; then send; then close this screen if still current.\tA send failure is retained as primary and closing this screen is still attempted.\tA close failure leaves the committed winner unchanged; later terminal paths remain NOOP.\tIf send and close both fail, throw the send failure and add the close failure as suppressed in execution order.\tEmit at most one CHOSEN InsightDecision; after either failure the offer remains terminal and cannot emit again.
            DECLINE\tInsightDecision.declined(triggerId)\tInsightOfferScreen\tsettle only when current offer triggerId matches\tAtomically commit settled triggerId and DECLINE as the immutable winning cause before sending the decision; then send; then close this screen if still current.\tA send failure is retained as primary and closing this screen is still attempted.\tA close failure leaves the committed winner unchanged; later terminal paths remain NOOP.\tIf send and close both fail, throw the send failure and add the close failure as suppressed in execution order.\tEmit at most one DECLINED InsightDecision; after either failure the offer remains terminal and cannot emit again.
            TIMEOUT\tInsightDecision.timedOut(triggerId)\tInsightOfferScreen\tsettle only when nowMs is greater than or equal to expiresAtMs for the same triggerId\tAtomically commit settled triggerId and TIMEOUT as the immutable winning cause before sending the decision; then send; then close this screen if still current.\tA send failure is retained as primary and closing this screen is still attempted.\tA close failure leaves the committed winner unchanged; later terminal paths remain NOOP.\tIf send and close both fail, throw the send failure and add the close failure as suppressed in execution order.\tEmit at most one TIMED_OUT InsightDecision; after either failure the offer remains terminal and cannot emit again.
            ESC\tInsightDecision.declined(triggerId)\tInsightOfferScreen.close\tsettle only when current offer triggerId matches\tAtomically commit settled triggerId and ESC as the immutable winning cause before sending the decline; then send; then close this screen if still current.\tA send failure is retained as primary and closing this screen is still attempted.\tA close or removal-hook failure leaves the committed winner unchanged; the later onRemoved terminal path is NOOP.\tIf send and close or removal both fail, throw the first failure and add later failures as suppressed in execution order.\tEmit at most one DECLINED InsightDecision; subsequent onRemoved callback emits nothing and cannot replace ESC as winner.
            REPLACED_BY_DIFFERENT_OFFER\tInsightDecision.declined(triggerId)\tInsightOfferScreen + InsightOfferScreenBootstrap + ScreenTransitionController.CurrentScreenCancellationHandler\tbootstrap compares outgoing and replacement triggerId before transition; handler settles the outgoing identity once\tAtomically commit the outgoing triggerId and REPLACED_BY_DIFFERENT_OFFER before sending its decline; then send; then switch to the different offer, which becomes authoritative only after the outgoing commit.\tA send failure is retained as primary and switching to the replacement is still attempted.\tA switch failure leaves the outgoing winner committed and the replacement non-authoritative; later terminal paths for the outgoing triggerId remain NOOP.\tIf send and switch both fail, throw the send failure and add the switch failure as suppressed in execution order.\tEmit at most one DECLINED InsightDecision for the outgoing triggerId; replacement cannot strand or re-settle the outgoing offer after either failure.
            REMOVED_EXCEPTIONALLY\tInsightDecision.declined(triggerId)\tInsightOfferScreen.onRemoved\tsettle the same triggerId when no prior terminal cause won\tAtomically commit settled triggerId and REMOVED_EXCEPTIONALLY before sending the decline; then send; then allow BongScreenBase removal cleanup and super.removed to continue.\tA send failure is retained as primary and later removal lifecycle stages are still attempted.\tA later removal-stage failure leaves the committed winner unchanged; later terminal paths remain NOOP.\tBongScreenBase throws the first failure and adds later removal failures as suppressed in execution order.\tEmit at most one DECLINED InsightDecision through the removal hook; exceptional removal cannot leave the offer unsettled or permit a retry.
            DUPLICATE_TERMINAL\tNOOP\tInsightOfferScreen\tsettled triggerId and first terminal cause are immutable\tObserve the already committed winner and return before decision send or any screen transition.\tNo decision send is attempted.\tNo screen switch is attempted by the duplicate path; the committed winner remains unchanged.\tNo new exception is produced by the duplicate path.\tEmit no second decision, perform no second transition, and do not mutate the winning cause.
            """.strip().lines().toList();
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
            social-new-identity\tSOCIAL_INVITE\tinvite-new\t1001\tNONE\tfalse\tORDINARY\tinventory\tNONE\tfalse\t1000\tDEFER_NOTIFY\tA new caller-owned identity resets notification eligibility.
            social-terminal\tSOCIAL_INVITE\tinvite-live\t1001\tNONE\tfalse\tSYSTEM_TERMINAL\tdeath\tDEATH\tfalse\t1000\tDEFER_NOTIFY\tA passive social offer never displaces a system terminal.
            hotkey-open\tHOTKEY\tidentity-screen\t9223372036854775807\tNONE\tfalse\tNONE\t\tNONE\tfalse\t1000\tOPEN\tAn immediate user keypress may open when no screen blocks it.
            hotkey-matching\tHOTKEY\tidentity-screen\t9223372036854775807\tNONE\tfalse\tORDINARY\tidentity-screen\tNONE\tfalse\t1000\tNOOP_MATCHING\tA matching screen is not recreated.
            hotkey-ordinary\tHOTKEY\tidentity-screen\t9223372036854775807\tNONE\tfalse\tORDINARY\tinventory\tNONE\tfalse\t1000\tBLOCK_DROP\tAn ordinary nonmatching screen consumes the physical moment; the keypress is not queued.
            hotkey-modal\tHOTKEY\tidentity-screen\t9223372036854775807\tNONE\tfalse\tMODAL\ttrade-offer\tNONE\tfalse\t1000\tBLOCK_DROP\tPhysical keypresses are never queued for future replay behind a modal.
            hotkey-terminal\tHOTKEY\tidentity-screen\t9223372036854775807\tNONE\tfalse\tSYSTEM_TERMINAL\tdeath\tDEATH\tfalse\t1000\tBLOCK_DROP\tA hotkey never displaces or waits behind a system terminal.
            insight-expired\tINSIGHT\tinsight-old\t999\tNONE\tfalse\tNONE\t\tNONE\tfalse\t1000\tEXPIRE\tAn expired insight settles through its domain owner and never opens.
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
            new FoundationRow("DiffListWidget", "update", "public UpdateResult update(List<? extends T> items)", "R7", "Null list/item/key and duplicate keys fail before mutation; equal ordered keys patch mounted rows; patch failure propagates unchanged, preserves the previous committed key sequence, and the next update retries the full list; structural key changes rebuild rows."),
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
                columns[5], columns[6], columns[7], columns[8], columns[9]
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
        String runtimeCardinality,
        String consumerRoute,
        String routeAnchor
    ) {
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

    private record KeybindingSourceContract(
        String sourceSite,
        String translationArgument,
        String inputTypeArgument,
        String defaultArgument,
        String categoryArgument
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
