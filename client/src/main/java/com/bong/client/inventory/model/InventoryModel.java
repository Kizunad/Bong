package com.bong.client.inventory.model;

import java.util.ArrayList;
import java.util.Collections;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

public final class InventoryModel {
    public static final int GRID_ROWS = 5;
    public static final int GRID_COLS = 7;
    public static final int HOTBAR_SIZE = 9;
    /** Legacy container ids — retained for backward compatibility with existing tests and fixture data. */
    public static final String PRIMARY_CONTAINER_ID = "main_pack";
    public static final String SMALL_POUCH_CONTAINER_ID = "small_pouch";
    public static final String FRONT_SATCHEL_CONTAINER_ID = "front_satchel";

    /** P4 — new container ids aligned with server schema (plan-backpack-equip-v1 P0). */
    public static final String BODY_POCKET_CONTAINER_ID = "body_pocket";
    public static final String BACK_PACK_CONTAINER_ID = "back_pack";

    /**
     * Container definition — stable id + display name + grid dimensions.
     *
     * <p>plan-tarkov-backpack-v1 P3（决议 #4）：{@code ownerInstanceId} = 该容器归属的穿戴背包件
     * instance_id（镜像 server ContainerSnapshotV1.owner_instance_id: Option&lt;u64&gt;）。仅
     * {@code pack_<id>} 派生容器有值；body_pocket / 静态容器为 {@code null}。client 双击穿戴背包件
     * 时直读 owner，免前缀解析。</p>
     */
    public record ContainerDef(String id, String name, int rows, int cols, Long ownerInstanceId) {
        public ContainerDef {
            Objects.requireNonNull(id, "id");
            Objects.requireNonNull(name, "name");
            if (id.isBlank()) throw new IllegalArgumentException("id must not be blank");
            if (rows <= 0 || cols <= 0) throw new IllegalArgumentException("invalid container size");
            if (ownerInstanceId != null && ownerInstanceId < 0) {
                throw new IllegalArgumentException("ownerInstanceId must be >= 0: " + ownerInstanceId);
            }
        }

        /** 无 owner 的容器（静态容器 / 旧 client 缺字段）。 */
        public ContainerDef(String id, String name, int rows, int cols) {
            this(id, name, rows, cols, null);
        }

        public ContainerDef(String name, int rows, int cols) {
            this(PRIMARY_CONTAINER_ID, name, rows, cols, null);
        }
    }

    /**
     * Default container layout (fallback when server snapshot is unavailable).
     * Aligned with plan-backpack-equip-v1 P0 schema: body_pocket (2×3) + back_pack (3×3).
     * Server pushes the authoritative list; this is only used before the first snapshot arrives.
     */
    public static final List<ContainerDef> DEFAULT_CONTAINERS = List.of(
        new ContainerDef(BODY_POCKET_CONTAINER_ID, "贴身口袋", 2, 3),
        new ContainerDef(BACK_PACK_CONTAINER_ID, "破草包", 3, 3)
    );

    private final List<ContainerDef> containers;
    private final List<GridEntry> gridItems;
    // plan-layered-equip-v1 P4（决议 #1/#12/#17）：装备槽内容由单件升级为分层 SlotContents（worn 栈 + held）。
    // equippedSlots = 完整分层态（面板渲染用）；equipped = 每槽代表件（held / worn 栈顶）兼容旧单件渲染路径
    //（护甲/手持模型同步、ArmorFeatureRenderer 等大量消费者不必改）。
    private final Map<EquipSlotType, SlotContents> equippedSlots;
    private final Map<EquipSlotType, InventoryItem> equipped;
    private final List<InventoryItem> hotbar;
    private final double currentWeight;
    private final double maxWeight;
    private final long boneCoins;
    private final String realm;
    private final double qiCurrent;
    private final double qiMax;
    private final double bodyLevel;

    private InventoryModel(
        List<ContainerDef> containers,
        List<GridEntry> gridItems,
        Map<EquipSlotType, SlotContents> equippedSlots,
        List<InventoryItem> hotbar,
        double currentWeight,
        double maxWeight,
        long boneCoins,
        String realm,
        double qiCurrent,
        double qiMax,
        double bodyLevel
    ) {
        this.containers = List.copyOf(containers);
        this.gridItems = List.copyOf(gridItems);
        EnumMap<EquipSlotType, SlotContents> slots = new EnumMap<>(EquipSlotType.class);
        EnumMap<EquipSlotType, InventoryItem> rep = new EnumMap<>(EquipSlotType.class);
        for (Map.Entry<EquipSlotType, SlotContents> e : equippedSlots.entrySet()) {
            SlotContents contents = e.getValue();
            if (contents == null || contents.isEmpty()) continue;
            slots.put(e.getKey(), contents);
            InventoryItem representative = contents.representative();
            if (representative != null) {
                rep.put(e.getKey(), representative);
            }
        }
        this.equippedSlots = Collections.unmodifiableMap(slots);
        this.equipped = Collections.unmodifiableMap(rep);
        this.hotbar = Collections.unmodifiableList(new ArrayList<>(hotbar));
        this.currentWeight = currentWeight;
        this.maxWeight = maxWeight;
        this.boneCoins = boneCoins;
        this.realm = Objects.requireNonNull(realm, "realm");
        this.qiCurrent = qiCurrent;
        this.qiMax = qiMax;
        this.bodyLevel = bodyLevel;
    }

    public static InventoryModel empty() {
        List<InventoryItem> emptyHotbar = new ArrayList<>(HOTBAR_SIZE);
        for (int i = 0; i < HOTBAR_SIZE; i++) {
            emptyHotbar.add(null);
        }
        return new InventoryModel(
            DEFAULT_CONTAINERS,
            List.of(),
            new EnumMap<>(EquipSlotType.class),
            emptyHotbar,
            0.0, 50.0, 0,
            "", 0.0, 100.0, 0.0
        );
    }

    /** plan-layered-equip-v1 P4：完整分层装备态（worn 栈 + held），供 EquipmentPanel 渲染。 */
    public Map<EquipSlotType, SlotContents> equippedSlots() {
        return equippedSlots;
    }

    public List<ContainerDef> containers() {
        return containers;
    }

    public static Builder builder() {
        return new Builder();
    }

    public List<GridEntry> gridItems() {
        return gridItems;
    }

    public Map<EquipSlotType, InventoryItem> equipped() {
        return equipped;
    }

    public List<InventoryItem> hotbar() {
        return hotbar;
    }

    public double currentWeight() {
        return currentWeight;
    }

    public double maxWeight() {
        return maxWeight;
    }

    public long boneCoins() {
        return boneCoins;
    }

    public String realm() {
        return realm;
    }

    public double qiCurrent() {
        return qiCurrent;
    }

    public double qiMax() {
        return qiMax;
    }

    public double qiFillRatio() {
        return qiMax > 0 ? Math.min(1.0, qiCurrent / qiMax) : 0.0;
    }

    public double bodyLevel() {
        return bodyLevel;
    }

    public boolean isEmpty() {
        if (!gridItems.isEmpty() || !equipped.isEmpty() || !realm.isEmpty()) {
            return false;
        }

        for (InventoryItem item : hotbar) {
            if (item != null && !item.isEmpty()) {
                return false;
            }
        }

        return true;
    }

    public record GridEntry(InventoryItem item, String containerId, int row, int col) {
        public GridEntry {
            Objects.requireNonNull(item, "item");
            Objects.requireNonNull(containerId, "containerId");
            if (containerId.isBlank()) throw new IllegalArgumentException("containerId must not be blank");
            if (row < 0) throw new IllegalArgumentException("row must be >= 0: " + row);
            if (col < 0) throw new IllegalArgumentException("col must be >= 0: " + col);
        }

        public GridEntry(InventoryItem item, int row, int col) {
            this(item, PRIMARY_CONTAINER_ID, row, col);
        }
    }

    public static final class Builder {
        private List<ContainerDef> containers = new ArrayList<>(DEFAULT_CONTAINERS);
        private final List<GridEntry> gridItems = new ArrayList<>();
        private final EnumMap<EquipSlotType, SlotContents> equippedSlots = new EnumMap<>(EquipSlotType.class);
        private final InventoryItem[] hotbar = new InventoryItem[HOTBAR_SIZE];
        private double currentWeight = 0.0;
        private double maxWeight = 50.0;
        private long boneCoins = 0;
        private String realm = "";
        private double qiCurrent = 0.0;
        private double qiMax = 100.0;
        private double bodyLevel = 0.0;
        private String primaryContainerId = DEFAULT_CONTAINERS.get(0).id();

        private Builder() {}

        /** Override default containers (e.g. from server data). */
        public Builder containers(List<ContainerDef> defs) {
            this.containers = defs == null || defs.isEmpty()
                ? new ArrayList<>(DEFAULT_CONTAINERS)
                : new ArrayList<>(defs);
            this.primaryContainerId = this.containers.isEmpty()
                ? BODY_POCKET_CONTAINER_ID
                : this.containers.get(0).id();
            return this;
        }

        public Builder gridItem(InventoryItem item, String containerId, int row, int col) {
            gridItems.add(new GridEntry(item, containerId, row, col));
            return this;
        }

        public Builder gridItem(InventoryItem item, int row, int col) {
            return gridItem(item, primaryContainerId, row, col);
        }

        /**
         * 单件装备便捷入口（兼容旧调用）：手槽 → held 单件，身体槽 → worn 单层栈。
         * 多层 / held+worn 混合用 {@link #equipSlot(EquipSlotType, SlotContents)}。
         */
        public Builder equip(EquipSlotType slot, InventoryItem item) {
            if (slot == null) return this;
            if (item == null || item.isEmpty()) {
                equippedSlots.remove(slot);
                return this;
            }
            SlotContents contents = slot.isHand()
                ? SlotContents.ofHeld(item)
                : SlotContents.ofWorn(item);
            equippedSlots.put(slot, contents);
            return this;
        }

        /** plan-layered-equip-v1 P4：直接设整槽分层内容（worn 栈 + held）。 */
        public Builder equipSlot(EquipSlotType slot, SlotContents contents) {
            if (slot == null) return this;
            if (contents == null || contents.isEmpty()) {
                equippedSlots.remove(slot);
            } else {
                equippedSlots.put(slot, contents);
            }
            return this;
        }

        public Builder hotbar(int index, InventoryItem item) {
            if (index >= 0 && index < HOTBAR_SIZE) {
                hotbar[index] = item;
            }
            return this;
        }

        public Builder weight(double current, double max) {
            this.currentWeight = current;
            this.maxWeight = max;
            return this;
        }

        public Builder boneCoins(long value) {
            this.boneCoins = value;
            return this;
        }

        public Builder cultivation(String realm, double qiCurrent, double qiMax, double bodyLevel) {
            this.realm = realm == null ? "" : realm;
            this.qiCurrent = qiCurrent;
            this.qiMax = qiMax;
            this.bodyLevel = bodyLevel;
            return this;
        }

        public InventoryModel build() {
            List<InventoryItem> hotbarList = new ArrayList<>(HOTBAR_SIZE);
            for (int i = 0; i < HOTBAR_SIZE; i++) {
                hotbarList.add(hotbar[i]);
            }
            return new InventoryModel(
                containers, gridItems, equippedSlots, hotbarList,
                currentWeight, maxWeight, boneCoins,
                realm, qiCurrent, qiMax, bodyLevel
            );
        }
    }
}
