package com.bong.client.tsy;

public record RiftPortalView(
    long entityId,
    String kind,
    String familyId,
    double x,
    double y,
    double z,
    int currentExtractTicks,
    Long activationWindowEnd
) {
    public boolean isExpired(long serverTick) {
        return activationWindowEnd != null && serverTick > activationWindowEnd;
    }
}
