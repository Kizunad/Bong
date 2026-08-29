package com.bong.client.ui;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class R7BootstrapInventoryContractTest {
    private static final Path CLIENT_SOURCE = R7SourceScan.productionRoot().resolve("BongClient.java");
    private static final Pattern EMPTY_REGISTER = Pattern.compile(
        "(?m)^\\s*([A-Za-z_$][\\w$]*(?:\\.[A-Za-z_$][\\w$]*)*)"
            + "\\.(register(?:[A-Z][\\w$]*)?|registerDefaults)\\(\\)\\s*;");

    @Test
    void uiBootstrapFixtureMatchesTheSourceDerivedRegistrationOrder() throws IOException {
        List<BootstrapRow> expected = readFixture();
        List<String> sourceCalls = sourceRegisterCalls();
        List<String> expectedCalls = expected.stream().map(BootstrapRow::sourceCall).toList();

        assertEquals(30, expected.size(), "UI bootstrap fixture must retain every registered UI/HUD/keybind module");
        assertEquals(expected.size(), new HashSet<>(expected.stream().map(BootstrapRow::moduleId).toList()).size(),
            "bootstrap module ids must be unique");
        assertEquals(expected.stream().map(BootstrapRow::order).toList(),
            java.util.stream.IntStream.rangeClosed(1, expected.size()).boxed().toList(),
            "bootstrap order must be contiguous and explicit");
        assertEquals(expectedCalls, expectedCalls.stream()
            .filter(sourceCalls::contains)
            .toList(),
            "every fixture source call must exist in production BongClient");

        List<String> actualUiCalls = sourceCalls.stream()
            .filter(expectedCalls::contains)
            .toList();
        assertEquals(expectedCalls, actualUiCalls,
            "UI bootstrap calls must retain their source order; unrelated render/audio calls may remain between them");
        assertTrue(sourceCalls.indexOf("BongNetworkHandler.register()") >= 0,
            "network registration must remain explicit in BongClient");
        assertTrue(sourceCalls.indexOf("BongNetworkHandler.register()")
                < sourceCalls.indexOf(expectedCalls.get(0)),
            "R7 UI bootstrap must not move ahead of network channel registration");
        assertFalse(expected.stream().anyMatch(row -> row.sourceCall().contains("BongNetworkHandler")),
            "R7 UI bootstrap inventory cannot own the R6 network registration call");

        Set<String> knownGroups = Set.of("SCREEN", "HUD", "KEYBIND", "INPUT");
        assertTrue(expected.stream().allMatch(row -> knownGroups.contains(row.group())),
            "bootstrap rows must declare a bounded UI group");
        assertTrue(expected.stream()
                .filter(row -> row.group().equals("SCREEN") && !row.moduleId().equals("screen_transition"))
                .allMatch(row -> row.dependencies().equals("screen_transition")),
            "every Screen bootstrap must depend on the transition owner");
    }

    private static List<String> sourceRegisterCalls() throws IOException {
        String source = R7SourceScan.read(CLIENT_SOURCE);
        Matcher matcher = EMPTY_REGISTER.matcher(source);
        List<String> result = new ArrayList<>();
        while (matcher.find()) {
            result.add(matcher.group(1) + "." + matcher.group(2) + "()");
        }
        return result;
    }

    private static List<BootstrapRow> readFixture() throws IOException {
        List<BootstrapRow> result = new ArrayList<>();
        for (String line : resourceLines()) {
            if (!R7SourceScan.isFixtureDataLine(line)) {
                continue;
            }
            String[] fields = line.split("\\t", -1);
            assertEquals(5, fields.length, "malformed bootstrap fixture row: " + line);
            result.add(new BootstrapRow(
                Integer.parseInt(fields[0]), fields[1], fields[2], fields[3], fields[4]
            ));
        }
        return result;
    }

    private static List<String> resourceLines() throws IOException {
        try (var stream = R7BootstrapInventoryContractTest.class.getResourceAsStream(
            "/bong/ui/ui-bootstrap-modules.tsv")) {
            if (stream == null) {
                throw new AssertionError("missing R7 bootstrap fixture");
            }
            return new String(stream.readAllBytes(), StandardCharsets.UTF_8).lines().toList();
        }
    }

    private record BootstrapRow(int order, String moduleId, String group,
                                String sourceCall, String dependencies) {
    }
}
