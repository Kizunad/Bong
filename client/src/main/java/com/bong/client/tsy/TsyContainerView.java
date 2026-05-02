package com.bong.client.tsy;

public record TsyContainerView(
    long entityId,
    String kind,
    String familyId,
    double x,
    double y,
    double z,
    String locked,
    boolean depleted,
    String searchedByPlayerId
) {
    public boolean interactable() {
        return !depleted && (searchedByPlayerId == null || searchedByPlayerId.isBlank());
    }

    public double distanceSq(double px, double py, double pz) {
        double dx = px - x;
        double dy = py - y;
        double dz = pz - z;
        return dx * dx + dy * dy + dz * dz;
    }

    public String kindLabelZh() {
        return switch (kind == null ? "" : kind) {
            case "dry_corpse" -> "干尸";
            case "skeleton" -> "骨架";
            case "storage_pouch" -> "储物袋残骸";
            case "stone_casket" -> "石匣";
            case "relic_core" -> "法阵核心";
            default -> "容器";
        };
    }
}
