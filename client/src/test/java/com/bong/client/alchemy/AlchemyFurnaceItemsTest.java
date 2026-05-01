package com.bong.client.alchemy;

import org.junit.jupiter.api.Test;

import javax.imageio.ImageIO;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class AlchemyFurnaceItemsTest {
    private static final Path FANTIE_ICON = Path.of(
            "src", "main", "resources", "assets", "bong-client", "textures", "gui", "items", "furnace_fantie.png"
    );

    @Test
    void furnaceFantieIconExistsAs128SquarePng() throws IOException {
        assertTrue(AlchemyFurnaceItems.isFurnaceItem("furnace_fantie"));
        assertTrue(Files.isRegularFile(FANTIE_ICON), "furnace_fantie icon missing at " + FANTIE_ICON);

        var image = ImageIO.read(FANTIE_ICON.toFile());
        assertEquals(128, image.getWidth());
        assertEquals(128, image.getHeight());
    }
}
