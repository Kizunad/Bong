package com.bong.client.combat;

import com.bong.client.network.ClientRequestSender;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import com.google.gson.JsonObject;

import java.util.Objects;

/** 将养护意图适配到既有 C2S sender；Screen 不直接依赖网络设施。 */
public final class RepairClientIntentSink implements UiIntentSink<RepairIntent> {
    private final Transport transport;

    RepairClientIntentSink(Transport transport) {
        this.transport = Objects.requireNonNull(transport, "transport must not be null");
    }

    public static RepairClientIntentSink production() {
        return new RepairClientIntentSink(new Transport() {
            @Override
            public void sendWeapon(long instanceId, int stationX, int stationY, int stationZ) {
                ClientRequestSender.sendRepairWeapon(instanceId, stationX, stationY, stationZ);
            }

            @Override
            public void sendLegacy(String material) {
                JsonObject payload = new JsonObject();
                payload.addProperty("material", material);
                ClientRequestSender.send("combat.repair_weapon", payload);
            }
        });
    }

    @Override
    public UiIntentResult dispatch(RepairIntent intent) {
        if (intent == null) {
            return UiIntentResult.rejected("repair intent must not be null");
        }
        try {
            if (intent instanceof RepairIntent.Commit commit) {
                if (commit.weaponInstanceId() > 0L) {
                    transport.sendWeapon(
                        commit.weaponInstanceId(),
                        commit.stationX(),
                        commit.stationY(),
                        commit.stationZ()
                    );
                } else {
                    transport.sendLegacy(commit.material());
                }
                return UiIntentResult.accepted();
            }
            throw new IllegalStateException("unsupported repair intent: " + intent.getClass().getName());
        } catch (RuntimeException failure) {
            String detail = failure.getMessage();
            return UiIntentResult.error("repair transport failed: "
                + (detail == null || detail.isBlank() ? failure.getClass().getSimpleName() : detail));
        }
    }

    interface Transport {
        void sendWeapon(long instanceId, int stationX, int stationY, int stationZ);

        void sendLegacy(String material);
    }
}
