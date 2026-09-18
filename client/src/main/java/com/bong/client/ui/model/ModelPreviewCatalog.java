package com.bong.client.ui.model;

import com.bong.client.armor.ArmorModelRegistry;
import com.bong.client.armor.WornPackModelRegistry;
import com.bong.client.block.BlockVanillaIconMap;
import com.bong.client.inspect.ItemInspectModel;
import com.bong.client.weapon.BongWeaponModelRegistry;
import net.minecraft.entity.EntityType;
import net.minecraft.entity.SpawnGroup;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;
import net.minecraft.registry.Registries;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.TreeSet;
import java.util.function.Supplier;

/** 从已注册资源建立目录；选择后才创建本地展示对象，从不向世界生成实体或物品。 */
public final class ModelPreviewCatalog {
    public enum Category {
        PLAYER("玩家"), CREATURE("生物"), ENTITY("场景实体"), ITEM("物品");
        public final String label;
        Category(String label) { this.label = label; }
    }

    public record Entry(String id, String label, Category category, EntityType<?> entityType,
                        Supplier<ItemInspectModel> item) {
        public static Entry player() { return new Entry("player", "当前玩家", Category.PLAYER, null, null); }
    }

    private ModelPreviewCatalog() {}

    public static List<Entry> entries() {
        var entries = new ArrayList<Entry>();
        entries.add(Entry.player());
        for (var type : Registries.ENTITY_TYPE) {
            var id = Registries.ENTITY_TYPE.getId(type);
            if (type == EntityType.PLAYER || type == EntityType.MARKER || type == EntityType.ITEM) continue;
            boolean creature = type.getSpawnGroup() != SpawnGroup.MISC
                || id.toString().equals("bong:whale") || id.toString().equals("bong:baolongwang");
            String label = type.getName().getString();
            if (label.startsWith("entity.")) label = id.getPath();
            entries.add(new Entry(id.toString(), label, creature ? Category.CREATURE : Category.ENTITY, type, null));
        }
        var templates = new TreeSet<>(BongWeaponModelRegistry.templateIds());
        templates.addAll(BlockVanillaIconMap.templateIds());
        templates.addAll(WornPackModelRegistry.templateIds());
        ArmorModelRegistry.all().forEach(spec -> templates.add(spec.templateId()));
        templates.forEach(id -> entries.add(new Entry("bong:" + id, id, Category.ITEM, null,
            () -> ItemInspectModel.find(id).orElseThrow(() -> new IllegalStateException("缺少物品模型：" + id)))));
        for (var item : Registries.ITEM) {
            if (item == Items.AIR) continue;
            var id = Registries.ITEM.getId(item);
            entries.add(new Entry(id.toString(), item.getName().getString(), Category.ITEM, null,
                () -> ItemInspectModel.stack(new ItemStack(item))));
        }
        entries.sort(Comparator.comparing(Entry::category).thenComparing(Entry::id));
        return List.copyOf(entries);
    }
}
