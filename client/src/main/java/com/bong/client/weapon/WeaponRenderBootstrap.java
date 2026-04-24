package com.bong.client.weapon;

import dev.felnull.specialmodelloader.api.event.SpecialModelLoaderEvents;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * 武器渲染 bootstrap —— plan-weapon-v1 路径 X 的 client-side 挂接点。
 *
 * SML 默认只接管自己声明过 LOAD_SCOPE 的 resource location。Bong 的 item model 放在
 * bong 命名空间，vanilla 原有 item（如 minecraft:iron_sword）若要被我们的 OBJ override，
 * 必须显式把对应 model location 纳入 scope，否则 SML 不介入加载链，仍走 vanilla cuboid 解析器。
 *
 * 当前实现通过 {@link BongWeaponModelRegistry} 统一维护 template → 宿主 item / model 资源的映射。
 */
public final class WeaponRenderBootstrap {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-weapon-render");
    private static final String BONG_NS = "bong";
    private static final String VANILLA_NS = "minecraft";

    private WeaponRenderBootstrap() {}

    public static void register() {
        SpecialModelLoaderEvents.LOAD_SCOPE.register(WeaponRenderBootstrap::isBongManagedModel);
        LOGGER.info(
            "SML LOAD_SCOPE registered: {}:* + vanilla overrides: {}",
            BONG_NS,
            BongWeaponModelRegistry.vanillaModelPaths()
        );
    }

    private static boolean isBongManagedModel(Identifier location) {
        if (BONG_NS.equals(location.getNamespace())) {
            return true;
        }
        return VANILLA_NS.equals(location.getNamespace())
            && BongWeaponModelRegistry.vanillaModelPaths().contains(location.getPath());
    }
}
