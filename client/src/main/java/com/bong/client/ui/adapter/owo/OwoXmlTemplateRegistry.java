package com.bong.client.ui.adapter.owo;

import io.wispforest.owo.ui.parsing.UIModel;
import io.wispforest.owo.ui.parsing.UIModelLoader;
import net.minecraft.util.Identifier;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.function.Function;

/**
 * 本地 owo XML 模板白名单。
 *
 * <p>server/agent 只传 semantic template_id；真正的 XML 只能从 client
 * 资源包里加载，避免远端输入直接变成组件树。</p>
 */
public final class OwoXmlTemplateRegistry {
    public static final Identifier CRAFT = new Identifier("bong", "craft");
    public static final Identifier CRAFT_COMPACT = new Identifier("bong", "craft-compact");
    public static final Identifier TERMINATE = new Identifier("bong", "terminate");
    public static final Identifier COFFIN_MENU = new Identifier("bong", "coffin-menu");
    public static final Identifier REPAIR = new Identifier("bong", "repair");
    public static final Identifier DEATH = new Identifier("bong", "death");
    public static final Identifier FORGE_CARRIER = new Identifier("bong", "forge-carrier");

    private static final Map<String, Identifier> PRODUCTION_TEMPLATES = Map.of(
        "craft", CRAFT,
        "craft-compact", CRAFT_COMPACT,
        "terminate", TERMINATE,
        "coffin-menu", COFFIN_MENU,
        "repair", REPAIR,
        "death", DEATH,
        "forge-carrier", FORGE_CARRIER
    );
    private static final OwoXmlTemplateRegistry PRODUCTION = new OwoXmlTemplateRegistry(
        UIModelLoader::get,
        PRODUCTION_TEMPLATES
    );

    private final Function<Identifier, UIModel> loader;
    private final Map<String, Identifier> templates;

    OwoXmlTemplateRegistry(Function<Identifier, UIModel> loader, Map<String, Identifier> templates) {
        this.loader = Objects.requireNonNull(loader, "loader must not be null");
        Objects.requireNonNull(templates, "templates must not be null");
        Map<String, Identifier> checked = new LinkedHashMap<>();
        templates.forEach((templateId, identifier) -> {
            if (templateId == null || templateId.isBlank()) {
                throw new IllegalArgumentException("template id must not be blank");
            }
            checked.put(templateId, Objects.requireNonNull(identifier, "template identifier must not be null"));
        });
        this.templates = Map.copyOf(checked);
    }

    /** 返回唯一生产模板注册表；不接受运行时 XML 或 URL。 */
    public static OwoXmlTemplateRegistry production() {
        return PRODUCTION;
    }

    public Set<String> templateIds() {
        return templates.keySet();
    }

    public boolean isRegistered(String templateId) {
        return templateId != null && templates.containsKey(templateId.strip());
    }

    /** 解析并校验本地模板；未知 id 或缺失资源都 fail-fast。 */
    public UIModel require(String templateId) {
        String normalized = normalize(templateId);
        Identifier identifier = templates.get(normalized);
        if (identifier == null) {
            throw new IllegalArgumentException("unregistered owo XML template: " + normalized);
        }
        UIModel model = loader.apply(identifier);
        if (model == null) {
            throw new IllegalStateException("missing owo XML resource: " + identifier);
        }
        return model;
    }

    public Identifier identifierFor(String templateId) {
        String normalized = normalize(templateId);
        Identifier identifier = templates.get(normalized);
        if (identifier == null) {
            throw new IllegalArgumentException("unregistered owo XML template: " + normalized);
        }
        return identifier;
    }

    private static String normalize(String templateId) {
        Objects.requireNonNull(templateId, "template id must not be null");
        String normalized = templateId.strip();
        if (normalized.isEmpty()) {
            throw new IllegalArgumentException("template id must not be blank");
        }
        return normalized;
    }
}
