package com.bong.client.inventory;

import org.junit.jupiter.api.Test;

import javax.imageio.ImageIO;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class GeneratedItemIconAssetsTest {
    private static final Path ITEM_TEXTURE_DIR = Path.of(
        "src", "main", "resources", "assets", "bong-client", "textures", "gui", "items"
    );

    @Test
    void firstBatchGeneratedIconsAre128RgbaPngs() throws Exception {
        for (String itemId : java.util.List.of("bone_coin_5", "shu_gu", "kaimai_dan", "hoe_xuantie", "array_flag")) {
            Path path = ITEM_TEXTURE_DIR.resolve(itemId + ".png");
            assertTrue(java.nio.file.Files.exists(path), itemId + " icon should exist");
            var image = ImageIO.read(path.toFile());
            assertNotNull(image, itemId + " icon should be readable");
            assertEquals(128, image.getWidth(), itemId + " icon width");
            assertEquals(128, image.getHeight(), itemId + " icon height");
            assertTrue(image.getColorModel().hasAlpha(), itemId + " icon should keep transparent alpha");
        }
    }
}
