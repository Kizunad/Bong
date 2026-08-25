package com.bong.client.ui;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * P0R source gate for the library-neutral UI boundary.
 *
 * <p>The existing Screen classes are intentionally outside this gate: they are
 * migration inventory, not evidence that the new neutral packages already
 * exist. Once a neutral package is added, every Java source below it is checked
 * against the same fixture.</p>
 */
class R7UiDependencyContractTest {
    private static final Path PRODUCTION_ROOT = R7SourceScan.productionRoot();

    private static final Map<String, List<String>> FORBIDDEN_TOKENS = Map.ofEntries(
        Map.entry("owo", List.of("io.wispforest.owo", "BaseOwoScreen", "FlowLayout")),
        Map.entry("vanilla_widget", List.of("net.minecraft.client.gui.widget", "ClickableWidget")),
        Map.entry("MCEF", List.of("MCEF", "mcef")),
        Map.entry("JCEF", List.of("JCEF", "jcef", "org.cef")),
        Map.entry("net.minecraft.client", List.of("net.minecraft.client")),
        Map.entry("ClientRequestSender", List.of("ClientRequestSender")),
        Map.entry("ClientRequestProtocol", List.of("ClientRequestProtocol")),
        Map.entry("ServerDataHandler", List.of("ServerDataHandler")),
        Map.entry("ProtoServerDataBridge", List.of("ProtoServerDataBridge")),
        Map.entry("ServerDataRouter", List.of("ServerDataRouter")),
        Map.entry("ServerDataEnvelope", List.of("ServerDataEnvelope")),
        Map.entry("Screen", List.of("net.minecraft.client.gui.screen.Screen", "extends Screen")),
        Map.entry("Store_static_fields", List.of(".STORE", "Store.", "Store.get", "Store.set")),
        Map.entry("network_handler", List.of("ServerDataHandler", "ProtoServerDataBridge", "ServerDataRouter")),
        Map.entry("reflection", List.of("java.lang.reflect", "Class.forName", "getDeclaredMethod")),
        Map.entry("annotation-discovery", List.of("org.reflections", "ServiceLoader")),
        Map.entry("network-registration", List.of("BongNetworkHandler.register"))
    );

    @Test
    void neutralPackagesObeyTheDependencyFixture() throws IOException {
        List<DependencyRule> rules = readRules();
        assertTrue(!rules.isEmpty(), "R7 dependency allowlist fixture must contain rules");

        for (DependencyRule rule : rules) {
            if (!isEnforcedProductionScope(rule.scope())) {
                continue;
            }
            Path scopeRoot = scopeRoot(rule.scope());
            if (!Files.exists(scopeRoot)) {
                continue;
            }
            try (var files = Files.walk(scopeRoot)) {
                for (Path source : files.filter(Files::isRegularFile)
                    .filter(path -> path.getFileName().toString().endsWith(".java"))
                    .toList()) {
                    String content = R7SourceScan.read(source);
                    for (String forbidden : rule.forbidden().split(";")) {
                        List<String> tokens = FORBIDDEN_TOKENS.get(forbidden);
                        assertTrue(tokens != null, "fixture has no source-gate mapping for " + forbidden);
                        for (String token : tokens) {
                            assertFalse(content.contains(token),
                                "R7 source gate violation in " + source + ": scope "
                                    + rule.scope() + " forbids " + forbidden + " (" + token + ")");
                        }
                    }
                }
            }
        }
    }

    @Test
    void legacyScreenScopeIsExplicitlyInventoryOnlyDuringP0R() throws IOException {
        List<DependencyRule> rules = readRules();
        DependencyRule legacyScreenRule = rules.stream()
            .filter(rule -> rule.scope().equals("client/*Screen.java"))
            .findFirst()
            .orElseThrow(() -> new AssertionError("fixture must declare the legacy Screen migration scope"));
        assertTrue(legacyScreenRule.rationale().contains("migration"),
            "legacy Screen scope must explain why it is not part of the neutral source gate");

        for (String neutralScope : List.of("ui/contract/**", "ui/state/**", "ui/intent/**", "ui/bootstrap/**")) {
            Path neutralRoot = scopeRoot(neutralScope);
            if (!Files.exists(neutralRoot)) {
                continue;
            }
            try (var files = Files.walk(neutralRoot)) {
                assertTrue(files.noneMatch(path -> path.getFileName().toString().endsWith("Screen.java")),
                    "P0R must not silently add a Screen implementation under " + neutralScope);
            }
        }
    }

    private static boolean isEnforcedProductionScope(String scope) {
        return scope.startsWith("ui/");
    }

    private static Path scopeRoot(String scope) {
        String normalized = scope.replace("/**", "");
        return PRODUCTION_ROOT.resolve(normalized);
    }

    private static List<DependencyRule> readRules() throws IOException {
        List<DependencyRule> result = new ArrayList<>();
        for (String line : resourceLines()) {
            if (!R7SourceScan.isFixtureDataLine(line)) {
                continue;
            }
            String[] fields = line.split("\\t", -1);
            assertTrue(fields.length >= 5, "malformed dependency rule: " + line);
            result.add(new DependencyRule(fields[0], fields[1], fields[2], fields[3], fields[4]));
        }
        return result;
    }

    private static List<String> resourceLines() throws IOException {
        try (var stream = R7UiDependencyContractTest.class.getResourceAsStream(
            "/bong/ui/ui-dependency-allowlist.tsv")) {
            if (stream == null) {
                throw new AssertionError("missing R7 UI dependency allowlist fixture");
            }
            return new String(stream.readAllBytes(), java.nio.charset.StandardCharsets.UTF_8)
                .lines()
                .toList();
        }
    }

    private record DependencyRule(String scope, String allowed, String forbidden,
                                  String owner, String rationale) {
    }
}
