package com.bong.client.ui;

import com.sun.source.tree.AssignmentTree;
import com.sun.source.tree.BinaryTree;
import com.sun.source.tree.ExpressionTree;
import com.sun.source.tree.IdentifierTree;
import com.sun.source.tree.LiteralTree;
import com.sun.source.tree.MemberReferenceTree;
import com.sun.source.tree.MemberSelectTree;
import com.sun.source.tree.MethodInvocationTree;
import com.sun.source.tree.MethodTree;
import com.sun.source.tree.NewClassTree;
import com.sun.source.tree.ParenthesizedTree;
import com.sun.source.tree.Tree;
import com.sun.source.tree.VariableTree;
import com.sun.source.util.TreePath;
import com.sun.source.util.TreePathScanner;
import org.junit.jupiter.api.Test;

import javax.lang.model.element.Element;
import javax.lang.model.element.ExecutableElement;
import javax.lang.model.element.TypeElement;
import javax.lang.model.element.VariableElement;

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
    void fixtureLoaderSkipsPlainAndOrdinalPrefixedHeaders() {
        assertFalse(R7SourceScan.isFixtureDataLine("# owner\tinput_type"));
        assertFalse(R7SourceScan.isFixtureDataLine("0\t# owner\tinput_type"));
        assertTrue(R7SourceScan.isFixtureDataLine("owner\tKEYSYM\tT"));
    }

    @Test
    void foundationFixturePinsThreeNamedContractsAndOwnership() {
        List<FoundationRow> rows = foundationRows();
        Map<String, Long> components = histogram(rows.stream().map(FoundationRow::component).toList());

        assertEquals(Set.of(
            "BongKeybindRegistry",
            "ClientThreadMarshal",
            "ScreenOpenPolicy"
        ), components.keySet(), "P0 freezes the remaining shared helper contracts");
        assertEquals(20, rows.size(), "foundation signature inventory changed without an explicit P0 decision");
        assertEquals(20, Set.copyOf(rows).size(), "each frozen contract row must be unique");
        assertEquals(20, rows.stream()
            .map(row -> row.component() + "::" + row.symbol())
            .collect(java.util.stream.Collectors.toSet()).size(),
            "each frozen contract symbol must have one unambiguous signature row");
        assertEquals(expectedFoundationRows(), rows,
            "foundation fixture drifted: every signature, owner, and invariant must be explicitly re-decided");
        assertTrue(rows.stream().allMatch(row -> row.owner().equals("R7")),
            "all three remaining contract surfaces are R7-owned even when integration belongs to another track");
        assertTrue(rows.stream().anyMatch(row -> row.component().equals("ClientThreadMarshal")
            && row.invariant().contains("false enqueues once")),
            "marshal contract must freeze exactly-once inline/enqueue behavior");
    }

    @Test
    void planCarriesTheFrozenContractAndBoundaryAnchors() {
        String plan = R7SourceScan.read(PLAN);
        for (String anchor : List.of(
            "真实 Screen **29** 个",
            "现有 92 个 token（87 个 executable）",
            "BongScreenBase<R extends ParentComponent>",
            "DiffListWidget<T,K,C extends Component>",
            "BongKeybindRegistry",
            "ClientThreadMarshal",
            "ScreenOpenPolicy",
            "R2",
            "R6",
            "tab-first",
            "DEFER_NOTIFY",
            "普通 hotkey 不重放",
            "ZERO production behavior change"
        )) {
            assertTrue(plan.contains(anchor), "R7 P0 plan is missing frozen anchor: " + anchor);
        }
        assertTrue(plan.contains("R6 仍独占网络 receiver/bridge/router"),
            "R7 must not claim R6 network/router ownership");
        assertTrue(plan.contains("唯一 screen-level intake"),
            "Screen teardown and subscription ownership must remain explicit");
        assertTrue(plan.contains("SparringInviteScreenBootstrap.java")
                && plan.contains("ScreenOpenPolicy"),
            "P4/P6 must name the real social-invite bootstrap and its open-policy owner");
        assertTrue(plan.contains("server-authoritative combat snapshot"),
            "P4 social deferral must block on an authoritative combat-state input");
        assertTrue(plan.contains("stale A/duplicate callback 不影响 B")
                && plan.contains("exact offerId claim/compare-and-clear exactly-once"),
            "P4 must prove an older insight lifecycle cannot clear a newer offer");
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
            Map.entry("forge_open", "UNKNOWN"),
            Map.entry("extract_cancel", "U"),
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
            assertTrue(Files.isRegularFile(source), "migration production source is missing: " + source);
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
        List<String> fixtureLines = resourceLines("/bong/ui/screen-open-policy.tsv");
        assertEquals(expectedOpenPolicyFixtureLines(), fixtureLines,
            "all 35 raw policy vectors and every input/output field must be explicitly re-decided");
        List<OpenPolicyRow> rows = openPolicyRows();
        assertEquals(35, rows.size(), "ScreenOpenPolicy P0 decision vectors changed");
        assertEquals(35, rows.stream().map(OpenPolicyRow::scenario)
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
        assertEquals("OPEN", findPolicy(rows, "hotkey-combat-open").decision());
        assertEquals("OPEN", findPolicy(rows, "insight-open").decision());
        assertEquals("OPEN", findPolicy(rows, "insight-combat-open").decision());
        assertEquals("EXPIRE", findPolicy(rows, "insight-expired").decision());
        assertEquals("OPEN", findPolicy(rows, "death-combat-open").decision());
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
        List<InsightSettlementRow> rows = insightSettlementRows();
        assertEquals(8, rows.size(), "each bounded insight terminal path must have one contract row");
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
        assertTrue(rows.stream().anyMatch(row -> row.observableEffect().contains("stale timeout of offer A")
                && row.observableEffect().contains("offer B")),
            "the P4 contract must explicitly prohibit stale offer A from clearing newer current or pending offer B");
        assertTrue(rows.stream().allMatch(row -> !row.commitOrder().contains("send; then claim")),
            "every terminal cause must claim its exact offerId before any send or transition side effect");
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
        List<KeybindProductionSiteRow> expectedSites = expected;
        List<KeybindingSourceSite> actualSites = productionKeybindingSourceSites();
        assertEquals(26, actualSites.size(),
            "P4 requires every production keybinding site to use the global registry");
        assertEquals(26, expected.size(), "the production-site manifest must retain all 26 logical bindings");
        assertEquals(26, expectedSites.size(),
            "the registry-backed production subset must retain every logical binding");
        assertEquals(26, expected.stream().map(KeybindProductionSiteRow::ownerId)
            .collect(java.util.stream.Collectors.toSet()).size(),
            "every logical binding needs one globally unique BindingOwner id");
        Set<SourceSiteIdentity> expectedIdentities = expectedSites.stream()
            .map(row -> new SourceSiteIdentity(row.sourcePath(), row.sourceSite()))
            .collect(java.util.stream.Collectors.toCollection(TreeSet::new));
        Set<SourceSiteIdentity> actualIdentities = actualSites.stream()
            .map(site -> new SourceSiteIdentity(site.sourcePath(), site.sourceSite()))
            .collect(java.util.stream.Collectors.toCollection(TreeSet::new));
        assertEquals(expectedIdentities, actualIdentities,
            "fixture and production must match the exact (sourcePath, assignment target) set in both directions");
        assertEquals(resourceLines("/bong/ui/keybind-production-sites.tsv"),
            keybindProductionSiteRows().stream().map(KeybindProductionSiteRow::fixtureLine).toList(),
            "every production keybinding declaration must parse as one exact typed manifest row");
        assertEquals(34, actualSites.stream().mapToInt(KeybindingSourceSite::runtimeCardinality).sum(),
            "all 26 registry sites must expand to exactly 34 runtime bindings");
        Set<String> expandedTranslationKeys = new TreeSet<>();
        for (KeybindingSourceSite site : actualSites) {
            expandedTranslationKeys.addAll(site.expandedTranslationKeys());
        }
        assertEquals(34, expandedTranslationKeys.size(),
            "every registry-backed runtime binding must have a unique effective translation key");
        for (KeybindProductionSiteRow row : expectedSites) {
            KeybindingSourceSite actual = actualSites.stream()
                .filter(site -> site.sourcePath().equals(row.sourcePath())
                    && site.sourceSite().equals(row.sourceSite()))
                .findFirst()
                .orElseThrow(() -> new AssertionError("missing pinned source site for " + row.ownerId()));
            assertEquals(row.translationContract(), actual.translationContract(),
                "effective translation contract drifted for " + row.ownerId());
            assertEquals(row.inputType(), actual.inputType(),
                "effective input type drifted for " + row.ownerId());
            assertEquals(row.normalizedDefaultContract(), actual.defaultContract(),
                "effective default key drifted for " + row.ownerId());
            assertEquals(row.categoryContract(), actual.categoryContract(),
                "effective category drifted for " + row.ownerId());
            assertEquals(row.runtimeCardinalityCount(), actual.runtimeCardinality(),
                "pinned runtime cardinality drifted for " + row.ownerId());
            assertEquals(row.expandedTranslationKeys(), actual.expandedTranslationKeys(),
                "expanded runtime translation keys drifted for " + row.ownerId());
        }
        List<ExpandedProductionDefault> expandedDefaults = expandedProductionDefaults(
            expectedSites, actualSites, keybindRows()
        );
        assertEquals(34, expandedDefaults.size(),
            "the registry-site collision audit must inspect every expanded default");
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
            "the 26 expanded target defaults must not collide with frozen vanilla reservations");

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
                .orElseThrow(() -> new AssertionError("missing pinned source site for " + row.ownerId()));
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
                String source = R7SourceScan.read(root);
                for (String name : names) {
                    assertFalse(source.contains(name), "P0/R6 ownership violation in " + root + ": " + name);
                }
                return;
            }
            try (var files = Files.walk(root)) {
                for (Path path : files.filter(Files::isRegularFile)
                    .filter(candidate -> candidate.getFileName().toString().endsWith(".java"))
                    .toList()) {
                    String source = R7SourceScan.read(path);
                    for (String name : names) {
                        assertFalse(source.contains(name), "P0/R6 ownership violation in " + path + ": " + name);
                    }
                }
            }
        } catch (IOException exception) {
            throw new AssertionError("unable to scan R7 ownership boundary " + root, exception);
        }
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

    private static List<KeybindingSourceSite> productionKeybindingSourceSites() throws IOException {
        List<KeybindingSourceSite> result = new java.util.ArrayList<>();
        for (R7SourceScan.ParsedUnit parsed : R7SourceScan.parseJava(CLIENT_ROOT)) {
            new TreePathScanner<Void, Void>() {
                @Override
                public Void visitMethodInvocation(MethodInvocationTree tree, Void unused) {
                    Element method = parsed.trees().getElement(getCurrentPath());
                    if (method instanceof ExecutableElement executable
                        && executable.getSimpleName().contentEquals("register")
                        && executable.getEnclosingElement() instanceof TypeElement owner
                        && owner.getQualifiedName().contentEquals(
                            "com.bong.client.ui.BongKeybindRegistry"
                        )) {
                        assertEquals(1, tree.getArguments().size(),
                            "global registry register must receive one BindingSpec: " + parsed.path());
                        assertTrue(tree.getArguments().get(0) instanceof NewClassTree,
                            "global registry register must receive an explicit BindingSpec: " + parsed.path());
                        NewClassTree spec = (NewClassTree) tree.getArguments().get(0);
                        TreePath specPath = new TreePath(getCurrentPath(), spec);
                        Element constructor = parsed.trees().getElement(specPath);
                        assertTrue(constructor instanceof ExecutableElement
                                && constructor.getEnclosingElement() instanceof TypeElement specOwner
                                && specOwner.getQualifiedName().contentEquals(
                                    "com.bong.client.ui.BongKeybindRegistry.BindingSpec"),
                            "registry registration must construct BindingSpec: " + parsed.path());
                        assertEquals(5, spec.getArguments().size(),
                            "BindingSpec must carry owner/translation/type/default/category: " + parsed.path());
                        String sourceSite = enclosingAssignmentTarget(getCurrentPath());
                        assertNotNull(sourceSite,
                            "registry registration needs a stable assignment target in " + parsed.path());
                        result.add(new KeybindingSourceSite(
                            CLIENT_ROOT.relativize(parsed.path()).toString().replace('\\', '/'),
                            sourceSite,
                            translationContract(resolveTranslationKeys(
                                new TreePath(specPath, spec.getArguments().get(1)), parsed
                            )),
                            resolveInputType(new TreePath(specPath, spec.getArguments().get(2)), parsed),
                            resolveDefaultContract(new TreePath(specPath, spec.getArguments().get(3)), parsed),
                            resolveString(new TreePath(specPath, spec.getArguments().get(4)), parsed),
                            resolveTranslationKeys(new TreePath(specPath, spec.getArguments().get(1)), parsed).size(),
                            resolveTranslationKeys(new TreePath(specPath, spec.getArguments().get(1)), parsed)
                        ));
                    }
                    return super.visitMethodInvocation(tree, unused);
                }

                @Override
                public Void visitNewClass(NewClassTree tree, Void unused) {
                    TreePath constructorPath = getCurrentPath();
                    Element constructor = parsed.trees().getElement(constructorPath);
                    if (!(constructor instanceof ExecutableElement executable)
                        || !(executable.getEnclosingElement() instanceof TypeElement owner)
                        || !owner.getQualifiedName().contentEquals("net.minecraft.client.option.KeyBinding")) {
                        return super.visitNewClass(tree, unused);
                    }
                    assertEquals(4, tree.getArguments().size(),
                        "every production KeyBinding constructor overload must be modeled: " + tree);
                    assertProductionRegistration(constructorPath, parsed);
                    String sourceSite = enclosingAssignmentTarget(constructorPath);
                    assertNotNull(sourceSite, "KeyBinding constructor needs a stable assignment target in " + parsed.path());
                    List<String> translations = resolveTranslationKeys(
                        new TreePath(constructorPath, tree.getArguments().get(0)), parsed
                    );
                    result.add(new KeybindingSourceSite(
                        CLIENT_ROOT.relativize(parsed.path()).toString().replace('\\', '/'),
                        sourceSite,
                        translationContract(translations),
                        resolveInputType(new TreePath(constructorPath, tree.getArguments().get(1)), parsed),
                        resolveDefaultContract(new TreePath(constructorPath, tree.getArguments().get(2)), parsed),
                        resolveString(new TreePath(constructorPath, tree.getArguments().get(3)), parsed),
                        translations.size(),
                        translations
                    ));
                    return super.visitNewClass(tree, unused);
                }
            }.scan(parsed.unit(), null);
        }
        result.sort(java.util.Comparator
            .comparing(KeybindingSourceSite::sourcePath)
            .thenComparing(KeybindingSourceSite::sourceSite));
        return result;
    }

    private static void assertProductionRegistration(TreePath constructorPath, R7SourceScan.ParsedUnit parsed) {
        TreePath parent = constructorPath.getParentPath();
        assertTrue(parent != null && parent.getLeaf() instanceof MethodInvocationTree,
            "every KeyBinding constructor must be a direct registration argument in " + parsed.path());
        Element method = parsed.trees().getElement(parent);
        assertTrue(method instanceof ExecutableElement,
            "registration invocation must resolve to a real method in " + parsed.path());
        ExecutableElement executable = (ExecutableElement) method;
        TypeElement owner = (TypeElement) executable.getEnclosingElement();
        if (owner.getQualifiedName().contentEquals(
            "net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper")
            && executable.getSimpleName().contentEquals("registerKeyBinding")) {
            return;
        }
        assertTrue(executable.getSimpleName().contentEquals("apply")
                && parent.getLeaf() instanceof MethodInvocationTree invocation
                && invocation.getMethodSelect() instanceof MemberSelectTree select
                && parsed.trees().getElement(new TreePath(parent, select.getExpression()))
                    instanceof VariableElement registrar
                && registrar.asType().toString().equals(
                    "java.util.function.UnaryOperator<net.minecraft.client.option.KeyBinding>"),
            "constructor must use Fabric registration or an attributed UnaryOperator<KeyBinding> seam in "
                + parsed.path());
        assertTrue(hasFabricRegistrarCaller(parsed, enclosingMethodElement(parent, parsed)),
            "registrar seam must have a real caller passing KeyBindingHelper::registerKeyBinding in " + parsed.path());
    }

    private static ExecutableElement enclosingMethodElement(TreePath path, R7SourceScan.ParsedUnit parsed) {
        for (TreePath cursor = path; cursor != null; cursor = cursor.getParentPath()) {
            if (cursor.getLeaf() instanceof MethodTree) {
                Element element = parsed.trees().getElement(cursor);
                if (element instanceof ExecutableElement executable) {
                    return executable;
                }
            }
        }
        throw new AssertionError("registration seam lacks an enclosing method in " + parsed.path());
    }

    private static boolean hasFabricRegistrarCaller(
        R7SourceScan.ParsedUnit parsed,
        ExecutableElement seam
    ) {
        final boolean[] found = {false};
        new TreePathScanner<Void, Void>() {
            @Override
            public Void visitMethodInvocation(MethodInvocationTree invocation, Void unused) {
                if (seam.equals(parsed.trees().getElement(getCurrentPath()))
                    && invocation.getArguments().stream().anyMatch(argument ->
                        argument instanceof MemberReferenceTree reference
                            && reference.getQualifierExpression().toString().equals("KeyBindingHelper")
                            && reference.getName().contentEquals("registerKeyBinding"))) {
                    found[0] = true;
                }
                return super.visitMethodInvocation(invocation, unused);
            }
        }.scan(parsed.unit(), null);
        return found[0];
    }

    private static List<String> resolveTranslationKeys(TreePath path, R7SourceScan.ParsedUnit parsed) {
        ExpressionTree expression = (ExpressionTree) path.getLeaf();
        if (expression instanceof BinaryTree binary && binary.getKind() == Tree.Kind.PLUS) {
            String prefix = resolveString(new TreePath(path, binary.getLeftOperand()), parsed);
            ExpressionTree right = binary.getRightOperand();
            if (right instanceof ParenthesizedTree parenthesized) {
                right = parenthesized.getExpression();
            }
            assertEquals("i + 1", right.toString(),
                "only quick-slot translations may use runtime expansion");
            return java.util.stream.IntStream.rangeClosed(1, quickSlotCount())
                .mapToObj(index -> prefix + index)
                .toList();
        }
        return List.of(resolveString(path, parsed));
    }

    private static int quickSlotCount() {
        Path config = CLIENT_ROOT.resolve("combat/QuickSlotConfig.java");
        try {
            for (R7SourceScan.ParsedUnit parsed : R7SourceScan.parseJava(config.getParent())) {
                if (!parsed.path().equals(config)) {
                    continue;
                }
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
                }.scan(parsed.unit(), null);
                assertEquals(9, count[0], "QuickSlotConfig.SLOT_COUNT is the keybinding cardinality source");
                return count[0];
            }
        } catch (IOException exception) {
            throw new AssertionError("unable to read QuickSlotConfig", exception);
        }
        throw new AssertionError("missing QuickSlotConfig.SLOT_COUNT");
    }

    private static String resolveString(TreePath path, R7SourceScan.ParsedUnit parsed) {
        if (path.getLeaf() instanceof LiteralTree literal && literal.getValue() instanceof String value) {
            return value;
        }
        Element element = parsed.trees().getElement(path);
        if (element instanceof VariableElement variable && variable.getConstantValue() instanceof String value) {
            return value;
        }
        throw new AssertionError("unsupported attributed string expression: " + path.getLeaf());
    }

    private static String resolveInputType(TreePath path, R7SourceScan.ParsedUnit parsed) {
        Element element = parsed.trees().getElement(path);
        assertTrue(element instanceof VariableElement, "input type must resolve to an enum constant: " + path.getLeaf());
        VariableElement variable = (VariableElement) element;
        assertEquals("net.minecraft.client.util.InputUtil.Type",
            ((TypeElement) variable.getEnclosingElement()).getQualifiedName().toString(),
            "input type must be canonical Minecraft InputUtil.Type");
        return variable.getSimpleName().toString();
    }

    private static String resolveDefaultContract(TreePath path, R7SourceScan.ParsedUnit parsed) {
        ExpressionTree expression = (ExpressionTree) path.getLeaf();
        if (expression instanceof BinaryTree binary && binary.getKind() == Tree.Kind.PLUS) {
            assertEquals("GLFW.GLFW_KEY_F1", binary.getLeftOperand().toString());
            assertEquals("i", binary.getRightOperand().toString());
            assertEquals(9, quickSlotCount());
            return "F1..F9";
        }
        if (expression instanceof MethodInvocationTree invocation
            && invocation.getMethodSelect().toString().equals("InputUtil.UNKNOWN_KEY.getCode")) {
            return "UNKNOWN";
        }
        Element element = parsed.trees().getElement(path);
        if (element instanceof VariableElement variable) {
            Object value = variable.getConstantValue();
            if (value instanceof Integer code) {
                if (variable.getSimpleName().contentEquals("GLFW_KEY_UNKNOWN")) {
                    return "UNKNOWN";
                }
                String name = variable.getSimpleName().toString();
                if (name.startsWith("GLFW_KEY_")) {
                    return name.substring("GLFW_KEY_".length());
                }
                return glfwContract(code);
            }
        }
        throw new AssertionError("unsupported attributed default-key expression: " + expression);
    }

    private static String glfwContract(int code) {
        if (code >= 65 && code <= 90) {
            return Character.toString((char) code);
        }
        throw new AssertionError("unsupported production default key code " + code);
    }

    private static String enclosingAssignmentTarget(TreePath path) {
        for (TreePath cursor = path.getParentPath(); cursor != null; cursor = cursor.getParentPath()) {
            if (cursor.getLeaf() instanceof AssignmentTree assignment) {
                return assignment.getVariable().toString().replaceAll("\\s+", " ").trim();
            }
            if (cursor.getLeaf() instanceof VariableTree variable) {
                return variable.getName().toString();
            }
        }
        return null;
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
            hotkey-combat-open\tHOTKEY\tidentity-screen\t9223372036854775807\tNONE\tfalse\tNONE\t\tNONE\ttrue\t1000\tOPEN\tCombat does not globally block an ordinary hotkey when no screen is active.
            hotkey-matching\tHOTKEY\tidentity-screen\t9223372036854775807\tNONE\tfalse\tORDINARY\tidentity-screen\tNONE\tfalse\t1000\tNOOP_MATCHING\tA matching screen is not recreated.
            hotkey-ordinary\tHOTKEY\tidentity-screen\t9223372036854775807\tNONE\tfalse\tORDINARY\tinventory\tNONE\tfalse\t1000\tBLOCK_DROP\tAn ordinary nonmatching screen consumes the physical moment; the keypress is not queued.
            hotkey-modal\tHOTKEY\tidentity-screen\t9223372036854775807\tNONE\tfalse\tMODAL\ttrade-offer\tNONE\tfalse\t1000\tBLOCK_DROP\tPhysical keypresses are never queued for future replay behind a modal.
            hotkey-terminal\tHOTKEY\tidentity-screen\t9223372036854775807\tNONE\tfalse\tSYSTEM_TERMINAL\tdeath\tDEATH\tfalse\t1000\tBLOCK_DROP\tA hotkey never displaces or waits behind a system terminal.
            insight-expired\tINSIGHT\tinsight-old\t999\tNONE\tfalse\tNONE\t\tNONE\tfalse\t1000\tEXPIRE\tInsightOfferScreenBootstrap settles the expired offer instance identified by offer_id before screen creation and the policy never opens a screen.
            insight-open\tINSIGHT\tinsight-live\t1001\tNONE\tfalse\tNONE\t\tNONE\tfalse\t1000\tOPEN\tA live insight opens when no UI is active.
            insight-combat-open\tINSIGHT\tinsight-live\t1001\tNONE\tfalse\tNONE\t\tNONE\ttrue\t1000\tOPEN\tCombat does not globally block a live insight when no screen is active.
            insight-preempt\tINSIGHT\tinsight-live\t1001\tNONE\tfalse\tORDINARY\tinventory\tNONE\tfalse\t1000\tPREEMPT\tInsight may replace ordinary non-modal UI through transition arbitration.
            insight-matching\tINSIGHT\tinsight-live\t1001\tNONE\tfalse\tMODAL\tinsight-live\tNONE\tfalse\t1000\tNOOP_MATCHING\tThe same insight identity is not reopened.
            insight-modal-first\tINSIGHT\tinsight-live\t1001\tNONE\tfalse\tMODAL\ttrade-offer\tNONE\tfalse\t1000\tDEFER_NOTIFY\tInsight waits behind an equal or higher modal and notifies once.
            insight-modal-repeat\tINSIGHT\tinsight-live\t1001\tNONE\ttrue\tMODAL\ttrade-offer\tNONE\tfalse\t1000\tDEFER_SILENT\tRepeated blocked insight observation is silent.
            insight-terminal\tINSIGHT\tinsight-live\t1001\tNONE\tfalse\tSYSTEM_TERMINAL\tdeath\tDEATH\tfalse\t1000\tDEFER_NOTIFY\tInsight never displaces death or termination UI.
            death-open\tSYSTEM_TERMINAL\tdeath-1\t9223372036854775807\tDEATH\tfalse\tNONE\t\tNONE\tfalse\t1000\tOPEN\tA death terminal opens when no screen is active.
            death-combat-open\tSYSTEM_TERMINAL\tdeath-1\t9223372036854775807\tDEATH\tfalse\tNONE\t\tNONE\ttrue\t1000\tOPEN\tCombat does not globally block a system terminal when no screen is active.
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
            new FoundationRow("BongKeybindRegistry", "type", "public final class BongKeybindRegistry", "R7", "Registrations are explicit and inspectable; no reflection or annotation discovery."),
            new FoundationRow("BongKeybindRegistry", "global", "public static BongKeybindRegistry global()", "R7", "Production has one registry instance so bootstrap-local registries cannot bypass conflict detection."),
            new FoundationRow("BongKeybindRegistry", "constructor", "BongKeybindRegistry(UnaryOperator<KeyBinding> registrar, List<ReservedDefault> reservedDefaults, Set<ConflictExemption> exemptions)", "R7", "公开注入 seam 允许跨 package 行为测试和适配器提供 Fabric registrar、显式 vanilla reservation 与精确 exemption；生产 bootstrap 仍必须使用 global()。"),
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
        return resourceLines("/bong/ui/foundation-contract.tsv").stream()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new FoundationRow(columns[0], columns[1], columns[2], columns[3], columns[4]))
            .toList();
    }

    private static List<KeybindRow> keybindRows() {
        return resourceLines("/bong/ui/keybind-migration.tsv").stream()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new KeybindRow(
                columns[0], columns[1], columns[2], columns[3], columns[4], columns[5],
                columns[6], columns[7], columns[8]
            ))
            .toList();
    }

    private static List<OpenPolicyRow> openPolicyRows() {
        return resourceLines("/bong/ui/screen-open-policy.tsv").stream()
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
        return resourceLines("/bong/ui/keybind-reserved-defaults.tsv").stream()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new ReservedDefaultRow(columns[0], columns[1], columns[2], columns[3]))
            .toList();
    }

    private static List<ConflictExemptionRow> conflictExemptionRows() {
        return resourceLines("/bong/ui/keybind-conflict-exemptions.tsv").stream()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new ConflictExemptionRow(
                columns[0], columns[1], columns[2], columns[3], columns[4]
            ))
            .toList();
    }

    private static List<KeybindProductionSiteRow> keybindProductionSiteRows() {
        return resourceLines("/bong/ui/keybind-production-sites.tsv").stream()
            .map(line -> line.split("\\t", -1))
            .map(columns -> new KeybindProductionSiteRow(
                columns[0], columns[1], columns[2], columns[3], columns[4],
                columns[5], columns[6], columns[7]
            ))
            .toList();
    }

    private static List<InsightSettlementRow> insightSettlementRows() {
        return resourceLines("/bong/ui/insight-settlement.tsv").stream()
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
