package com.bong.client.identity;

import com.bong.client.ui.contract.UiIntent;

import java.util.Objects;

/** 身份面板允许提交的类型化动作。 */
public sealed interface IdentityPanelIntent extends UiIntent
    permits IdentityPanelIntent.NewIdentity, IdentityPanelIntent.RenameIdentity, IdentityPanelIntent.SwitchIdentity {

    /** 创建一个新身份；名称规范化由意图边界统一处理。 */
    record NewIdentity(String name) implements IdentityPanelIntent {
        public NewIdentity {
            name = normalizeName(name);
            requireName(name);
        }
    }

    /** 修改当前身份名称。 */
    record RenameIdentity(String name) implements IdentityPanelIntent {
        public RenameIdentity {
            name = normalizeName(name);
            requireName(name);
        }
    }

    /** 切换到指定身份实例。 */
    record SwitchIdentity(int identityId) implements IdentityPanelIntent {
        public SwitchIdentity {
            identityId = Math.max(0, identityId);
        }
    }

    static String command(IdentityPanelIntent intent) {
        Objects.requireNonNull(intent, "identity intent must not be null");
        if (intent instanceof NewIdentity create) {
            return "identity new " + create.name();
        }
        if (intent instanceof RenameIdentity rename) {
            return "identity rename " + rename.name();
        }
        if (intent instanceof SwitchIdentity toggle) {
            return "identity switch " + toggle.identityId();
        }
        throw new IllegalStateException("unsupported identity intent: " + intent.getClass().getName());
    }

    private static String normalizeName(String rawName) {
        return rawName == null ? "" : rawName.trim().replaceAll("\\s+", " ");
    }

    private static void requireName(String name) {
        if (name.isEmpty()) {
            throw new IllegalArgumentException("identity name must not be blank");
        }
    }
}
