package com.bong.client.hud;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** 锁住全量 HUD 盘点，避免新增 layer 或直绘 overlay 时绕过迁移登记。 */
class HudRenderRegistryTest {
    private static final Path CLIENT_ROOT = Path.of("").toAbsolutePath().normalize();

    @Test
    void coversEveryLayerOnceInEnumOrder() {
        List<HudRenderRegistry.SurfaceDefinition> surfaces = HudRenderRegistry.productionSurfaces();
        List<HudRenderLayer> registeredLayers = surfaces.stream()
            .flatMap(surface -> surface.layer().stream())
            .toList();

        assertEquals(List.of(HudRenderLayer.values()), registeredLayers,
            "每个 HudRenderLayer 必须恰好登记一次，且顺序必须与枚举定义一致");
    }

    @Test
    void keepsTheFiveKnownDirectOverlaysExplicit() {
        List<String> directOverlayIds = HudRenderRegistry.directOverlays().stream()
            .map(HudRenderRegistry.SurfaceDefinition::surfaceId)
            .sorted()
            .toList();

        assertEquals(
            List.of(
                "BAOMAI_V3_OVERLAY",
                "CRACK_READING_OVERLAY",
                "HALLUCINATION_OVERLAY",
                "RESONANCE_LOCK_OVERLAY",
                "VOID_EROSION_OVERLAY"
            ),
            directOverlayIds,
            "直绘 overlay 不属于 HudRenderCommand，必须单独登记，不能在全量迁移中漏掉"
        );
        assertEquals(
            HudRenderRegistry.RenderPath.DIRECT_OVERLAY,
            HudRenderRegistry.require(HudRenderLayer.HALLUCINATION).path(),
            "HALLUCINATION 枚举 layer 的当前生产表现仍由直绘 overlay 提交"
        );
    }

    @Test
    void longTermFixtureMatchesTheProductionRegistry() throws IOException {
        List<FixtureRow> actual = fixtureRows();
        List<FixtureRow> expected = HudRenderRegistry.productionSurfaces().stream()
            .map(FixtureRow::from)
            .toList();

        assertEquals(expected, actual,
            "ui-svg-hud-inventory.tsv 必须逐行反映生产 registry，新增或迁移 surface 时两处都要更新");
        assertTrue(actual.stream().allMatch(FixtureRow::hasExistingTestOwner),
            "每个 HUD surface 都必须保留可定位的测试 owner");
    }

    private static List<FixtureRow> fixtureRows() throws IOException {
        try (var input = HudRenderRegistryTest.class.getResourceAsStream("/bong/ui/ui-svg-hud-inventory.tsv")) {
            assertTrue(input != null, "缺少 HUD SVG 长期盘点 fixture");
            return new String(input.readAllBytes(), StandardCharsets.UTF_8).lines()
                .filter(line -> !line.isBlank() && !line.stripLeading().startsWith("#"))
                .map(FixtureRow::parse)
                .toList();
        }
    }

    private record FixtureRow(
        String surfaceId,
        String layer,
        String path,
        String presentation,
        String owner,
        String asset,
        String binding,
        String guiException,
        String testOwner
    ) {
        private static FixtureRow from(HudRenderRegistry.SurfaceDefinition surface) {
            return new FixtureRow(
                surface.surfaceId(),
                surface.layer().map(Enum::name).orElse(""),
                surface.path().name(),
                surface.presentation().name(),
                surface.owner(),
                surface.svgAssets().stream()
                    .map(HudRenderRegistry.SvgAsset::fixtureValue)
                    .collect(Collectors.joining(";")),
                surface.dynamicBinding(),
                surface.guiException(),
                surface.testOwner()
            );
        }

        private static FixtureRow parse(String row) {
            String[] fields = row.split("\\t", -1);
            assertEquals(9, fields.length, "HUD SVG inventory 行字段数错误: " + row);
            return new FixtureRow(
                fields[0],
                fields[1],
                fields[2],
                fields[3],
                fields[4],
                fields[5],
                fields[6],
                fields[7],
                fields[8]
            );
        }

        private boolean hasExistingTestOwner() {
            if (testOwner.isBlank()) {
                return false;
            }
            Path clientRoot = Files.isDirectory(CLIENT_ROOT.resolve("src"))
                ? CLIENT_ROOT
                : CLIENT_ROOT.resolve("client");
            return Files.exists(clientRoot.resolve("src/test/java/com/bong/client/hud/" + testOwner + ".java"))
                || Files.exists(clientRoot.resolve("src/test/java/com/bong/client/hud/svg/" + testOwner + ".java"));
        }
    }
}
