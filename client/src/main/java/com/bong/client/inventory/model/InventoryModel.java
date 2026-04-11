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

    /** Container definition — name + grid dimensions. */
    public record ContainerDef(String name, int rows, int cols) {
        public ContainerDef {
            Objects.requireNonNull(name, "name");
            if (rows <= 0 || cols <= 0) throw new IllegalArgumentException("invalid container size");
        }
    }

    /** Default container layout — server can override via builder. */
    public static final List<ContainerDef> DEFAULT_CONTAINERS = List.of(
        new ContainerDef("主背包", 5, 7),
        new ContainerDef("小口袋", 3, 3),
        new ContainerDef("前挂包", 3, 4)
    );

    private final List<ContainerDef> containers;
    private final List<GridEntry> gridItems;
    private final Map<EquipSlotType, InventoryItem> equipped;
    private final List<InventoryItem> hotbar;
    private final double currentWeight;
    private final double maxWeight;
    private final long spiritStones;
    private final String realm;
    private final double qiCurrent;
    private final double qiMax;
    private final double bodyLevel;

    private InventoryModel(
        List<ContainerDef> containers,
        List<GridEntry> gridItems,
        Map<EquipSlotType, InventoryItem> equipped,
        List<InventoryItem> hotbar,
        double currentWeight,
        double maxWeight,
        long spiritStones,
        String realm,
        double qiCurrent,
        double qiMax,
        double bodyLevel
    ) {
        this.containers = List.copyOf(containers);
        this.gridItems = List.copyOf(gridItems);
        this.equipped = Collections.unmodifiableMap(new EnumMap<>(equipped));
        this.hotbar = Collections.unmodifiableList(new ArrayList<>(hotbar));
        this.currentWeight = currentWeight;
        this.maxWeight = maxWeight;
        this.spiritStones = spiritStones;
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

    public long spiritStones() {
        return spiritStones;
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
        return realm.isEmpty();
    }

    public record GridEntry(InventoryItem item, int row, int col) {
        public GridEntry {
            Objects.requireNonNull(item, "item");
            if (row < 0) throw new IllegalArgumentException("row must be >= 0: " + row);
            if (col < 0) throw new IllegalArgumentException("col must be >= 0: " + col);
        }
    }

    public static final class Builder {
        private List<ContainerDef> containers = new ArrayList<>(DEFAULT_CONTAINERS);
        private final List<GridEntry> gridItems = new ArrayList<>();
        private final EnumMap<EquipSlotType, InventoryItem> equipped = new EnumMap<>(EquipSlotType.class);
        private final InventoryItem[] hotbar = new InventoryItem[HOTBAR_SIZE];
        private double currentWeight = 0.0;
        private double maxWeight = 50.0;
        private long spiritStones = 0;
        private String realm = "";
        private double qiCurrent = 0.0;
        private double qiMax = 100.0;
        private double bodyLevel = 0.0;

        private Builder() {}

        /** Override default containers (e.g. from server data). */
        public Builder containers(List<ContainerDef> defs) {
            this.containers = new ArrayList<>(defs);
            return this;
        }

        public Builder gridItem(InventoryItem item, int row, int col) {
            gridItems.add(new GridEntry(item, row, col));
            return this;
        }

        public Builder equip(EquipSlotType slot, InventoryItem item) {
            equipped.put(slot, item);
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

        public Builder spiritStones(long value) {
            this.spiritStones = value;
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
                containers, gridItems, equipped, hotbarList,
                currentWeight, maxWeight, spiritStones,
                realm, qiCurrent, qiMax, bodyLevel
            );
        }
    }
}
