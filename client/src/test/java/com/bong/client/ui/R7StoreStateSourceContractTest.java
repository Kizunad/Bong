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

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class R7StoreStateSourceContractTest {
    private static final Path PRODUCTION_ROOT = R7SourceScan.productionRoot();

    @Test
    void everyUiStoreSourceRowMapsToAProductionClassAndDeclaredAccessMode() throws IOException {
        List<StoreRow> rows = readFixture();
        assertEquals(rows.size(), new HashSet<>(rows.stream().map(StoreRow::fqcn).toList()).size(),
            "each UI Store source must be listed exactly once");
        assertTrue(rows.stream().allMatch(row -> Set.of("PUSH", "PULL_ON_OPEN", "PULL_ON_TICK").contains(row.mode())),
            "Store source mode must be an explicit bounded enum");

        List<String> uiConsumers = uiConsumerSources();
        for (StoreRow row : rows) {
            Path sourcePath = sourcePath(row.fqcn());
            assertTrue(Files.exists(sourcePath), "fixture Store class is missing from production: " + row.fqcn());
            String storeSource = R7SourceScan.read(sourcePath);
            String simpleName = row.fqcn().substring(row.fqcn().lastIndexOf('.') + 1);
            assertTrue(uiConsumers.stream().anyMatch(source -> source.contains(simpleName)),
                "Store row is not consumed by any production Screen or bootstrap: " + row.fqcn());
            String accessorName = row.snapshotSymbol().substring(0, row.snapshotSymbol().indexOf('('));
            assertTrue(storeSource.contains(accessorName + "("),
                "snapshot symbol drifted for " + row.fqcn() + ": " + row.snapshotSymbol());
            if (row.mode().equals("PUSH")) {
                assertEquals("addListener/removeListener", row.listenerApi(),
                    "PUSH source must pin a paired listener API: " + row.fqcn());
                assertTrue(storeSource.contains("addListener") && storeSource.contains("removeListener"),
                    "PUSH source has no complete listener pair: " + row.fqcn());
            } else {
                assertEquals("NONE", row.listenerApi(),
                    "pull source must not pretend to have a push listener: " + row.fqcn());
            }
        }
    }

    @Test
    void storeFixtureSeparatesSessionLifecycleFromUiReadMode() throws IOException {
        List<StoreRow> rows = readFixture();
        assertTrue(rows.stream().noneMatch(row -> row.snapshotSymbol().contains("clearOnDisconnect")),
            "R7 read source fixture must not claim ownership of R2 disconnect cleanup");
        assertTrue(rows.stream().anyMatch(row -> row.mode().equals("PUSH")),
            "fixture must retain at least one listener-backed source example");
        assertTrue(rows.stream().anyMatch(row -> row.mode().equals("PULL_ON_OPEN")),
            "fixture must retain pull-on-open sources for stores without listeners");
    }

    private static List<String> uiConsumerSources() throws IOException {
        List<String> result = new ArrayList<>();
        try (var files = Files.walk(PRODUCTION_ROOT)) {
            for (Path path : files.filter(Files::isRegularFile)
                .filter(candidate -> candidate.getFileName().toString().endsWith("Screen.java")
                    || candidate.getFileName().toString().endsWith("Bootstrap.java")
                    || candidate.getFileName().toString().endsWith("UiStateSource.java"))
                .toList()) {
                result.add(R7SourceScan.read(path));
            }
        }
        return result;
    }

    private static Path sourcePath(String fqcn) {
        String relative = fqcn.substring("com.bong.client.".length()).replace('.', '/') + ".java";
        return PRODUCTION_ROOT.resolve(relative);
    }

    private static List<StoreRow> readFixture() throws IOException {
        List<StoreRow> result = new ArrayList<>();
        for (String line : resourceLines()) {
            if (!R7SourceScan.isFixtureDataLine(line)) {
                continue;
            }
            String[] fields = line.split("\\t", -1);
            assertEquals(4, fields.length, "malformed Store source fixture row: " + line);
            result.add(new StoreRow(fields[0], fields[1], fields[2], fields[3]));
        }
        return result;
    }

    private static List<String> resourceLines() throws IOException {
        try (var stream = R7StoreStateSourceContractTest.class.getResourceAsStream(
            "/bong/ui/store-state-sources.tsv")) {
            if (stream == null) {
                throw new AssertionError("missing R7 Store source fixture");
            }
            return new String(stream.readAllBytes(), StandardCharsets.UTF_8).lines().toList();
        }
    }

    private record StoreRow(String fqcn, String mode, String snapshotSymbol, String listenerApi) {
    }
}
