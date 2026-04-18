package com.bong.client.weapon;

import dev.felnull.specialmodelloader.api.event.SpecialModelLoaderEvents;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.Set;

/**
 * 武器渲染 bootstrap —— plan-weapon-v1 路径 X 的 client-side 挂接点。
 *
 * SML 默认只接管自己声明过 LOAD_SCOPE 的 resource location。Bong 的 item model 放在
 * bong 命名空间，vanilla 原有 item（如 minecraft:iron_sword）若要被我们的 OBJ override，
 * 必须显式把对应 model location 纳入 scope，否则 SML 不介入加载链，仍走 vanilla cuboid 解析器。
 *
 * 当前实现是测试期骨架：
 *   - 打开 bong:* 所有 item model 走 SML
 *   - 打开 VANILLA_ITEM_OVERRIDES 里列出的 vanilla item，对应资产管线里 --override 目标。
 *     每次 tripo_to_sml.py 新 override 一个 vanilla item，就把对应路径加到这个 set。
 *
 * W5 正式完成后 —— Mixin 拦 HeldItemRenderer 按 WeaponEquippedStore 快照画模型（见 §5.1）
 * —— 对 vanilla item 的 scope 劫持可撤销。
 */
public final class WeaponRenderBootstrap {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-weapon-render");
    private static final String BONG_NS = "bong";
    private static final String VANILLA_NS = "minecraft";

    /** vanilla item model 路径白名单，resource location = "minecraft:<path>"。路径形式 "item/<name>"。 */
    private static final Set<String> VANILLA_ITEM_OVERRIDES = Set.of(
            "item/iron_sword",         // placeholder_sword（唐刀模型）
            "item/flint",              // crystal_shard_dagger（匕首模型）
            "item/nether_star",        // cracked_heart（碎裂的心，展示用）
            "item/totem_of_undying",   // wooden_totem（木制图腾，后期转 Entity）
            "item/netherite_sword",    // spirit_sword（灵剑 tier 1，cracked_sword mesh）
            "item/bone",               // bone_dagger（骨刀 tier 0）
            "item/diamond_sword",      // flying_sword_feixuan（飞玄剑 tier 2，medieval_dagger mesh）
            "item/leather"             // hand_wrap（拳套 tier 0，armored_gauntlets mesh）
    );

    private WeaponRenderBootstrap() {}

    public static void register() {
        SpecialModelLoaderEvents.LOAD_SCOPE.register(WeaponRenderBootstrap::isBongManagedModel);
        LOGGER.info("SML LOAD_SCOPE registered: {}:* + vanilla overrides: {}", BONG_NS, VANILLA_ITEM_OVERRIDES);
    }

    private static boolean isBongManagedModel(Identifier location) {
        if (BONG_NS.equals(location.getNamespace())) {
            return true;
        }
        return VANILLA_NS.equals(location.getNamespace()) && VANILLA_ITEM_OVERRIDES.contains(location.getPath());
    }
}
