package com.bong.client.iris;

import com.bong.client.BongClient;
import net.fabricmc.loader.api.FabricLoader;
import net.fabricmc.loader.api.ModContainer;

import java.util.Optional;

public final class BongIrisCompat {
    private static boolean available;
    private static String irisVersion = "";

    private BongIrisCompat() {
    }

    public static void init() {
        available = FabricLoader.getInstance().isModLoaded("iris");
        if (available) {
            Optional<ModContainer> container = FabricLoader.getInstance().getModContainer("iris");
            irisVersion = container
                    .map(c -> c.getMetadata().getVersion().getFriendlyString())
                    .orElse("unknown");
            BongClient.LOGGER.info("[BongIris] Iris detected v{}, uniform injection active", irisVersion);
        } else {
            BongClient.LOGGER.info("[BongIris] Iris not found, shader features disabled");
        }
    }

    public static boolean isAvailable() {
        return available;
    }

    public static String getIrisVersion() {
        return irisVersion;
    }
}
