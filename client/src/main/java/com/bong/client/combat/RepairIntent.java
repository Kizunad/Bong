package com.bong.client.combat;

import com.bong.client.ui.contract.UiIntent;

import java.util.Objects;

/** 武器养护界面允许发送的类型化意图。 */
public sealed interface RepairIntent extends UiIntent permits RepairIntent.Commit {
    /** 提交一次养护请求；正实例 ID 走结构化 wire，旧入口保留 material payload。 */
    record Commit(String material, long weaponInstanceId, int stationX, int stationY, int stationZ)
        implements RepairIntent {
        public Commit {
            Objects.requireNonNull(material, "repair material must not be null");
        }
    }
}
