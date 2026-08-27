package com.bong.client.ui.adapter.owo;

import com.bong.client.ui.contract.UiScreenScope;
import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.core.Component;
import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.core.ParentComponent;
import io.wispforest.owo.ui.parsing.UIModel;
import net.minecraft.text.Text;

import java.util.Objects;
import java.util.ArrayList;
import java.util.List;

/**
 * owo XML 的唯一生产 Screen 宿主。
 *
 * <p>模板由白名单 registry 选择，Screen 子类只实现状态绑定和 typed
 * intent wiring；组件树和布局不在 Java 中声明。</p>
 */
public abstract class OwoXmlScreenHost<R extends ParentComponent> extends BaseOwoScreen<R> {
    private final Class<R> rootType;
    private final String templateId;
    private final OwoXmlTemplateRegistry templates;
    private final OwoXmlHostLifecycle lifecycle = new OwoXmlHostLifecycle();
    private UIModel model;
    private String selectedTemplateId;

    protected OwoXmlScreenHost(
        Text title,
        Class<R> rootType,
        String templateId
    ) {
        this(title, rootType, templateId, OwoXmlTemplateRegistry.production());
    }

    OwoXmlScreenHost(
        Text title,
        Class<R> rootType,
        String templateId,
        OwoXmlTemplateRegistry templates
    ) {
        super(Objects.requireNonNull(title, "title must not be null"));
        this.rootType = Objects.requireNonNull(rootType, "rootType must not be null");
        this.templateId = Objects.requireNonNull(templateId, "templateId must not be null").strip();
        if (this.templateId.isEmpty()) {
            throw new IllegalArgumentException("templateId must not be blank");
        }
        this.templates = Objects.requireNonNull(templates, "templates must not be null");
    }

    @Override
    protected final OwoUIAdapter<R> createAdapter() {
        selectedTemplateId = selectTemplateId(width, height);
        model = templates.require(selectedTemplateId);
        return model.createAdapter(rootType, this);
    }

    /**
     * 根据当前逻辑 viewport 选择同一语义界面的本地布局变体。默认不分支；
     * 具体 Screen 只能返回白名单中的 template id。
     */
    protected String selectTemplateId(int logicalWidth, int logicalHeight) {
        return templateId;
    }

    @Override
    protected final void build(R root) {
        bindTemplate(root);
    }

    /** XML 已完成静态布局；子类只绑定动态 bridge 和交互。 */
    protected abstract void bindTemplate(R root);

    /** 可选的打开回调；监听器应登记到传入 scope。 */
    protected void onHostOpened(UiScreenScope scope) {
    }

    /** 可选的关闭回调；scope 已先关闭，late callback 会被拦截。 */
    protected void onHostClosed() {
    }

    @Override
    public void init() {
        String nextTemplateId = selectTemplateId(width, height);
        if (shouldReloadTemplate(uiAdapter != null, selectedTemplateId, nextTemplateId)) {
            // resize 跨过布局断点时必须重新装载 XML；同一变体内仍复用 owo 组件树。
            uiAdapter.dispose();
            uiAdapter = null;
        }
        super.init();
        lifecycle.openOnce(this::onHostOpened);
    }

    @Override
    public void tick() {
        super.tick();
        lifecycle.tick(System.currentTimeMillis());
    }

    @Override
    public void removed() {
        Throwable primary = null;
        try {
            lifecycle.close(this::onHostClosed);
        } catch (Throwable failure) {
            primary = failure;
        }
        try {
            super.removed();
        } catch (Throwable failure) {
            primary = OwoXmlHostLifecycle.appendFailure(primary, failure);
        }
        OwoXmlHostLifecycle.throwIfPresent(primary);
    }

    protected final UiScreenScope screenScope() {
        return lifecycle.scope();
    }

    protected final <C extends Component> C component(Class<C> expectedClass, String id) {
        if (uiAdapter == null) {
            throw new IllegalStateException("XML component tree is not initialized");
        }
        return requireComponent(uiAdapter.rootComponent, expectedClass, id);
    }

    static boolean shouldReloadTemplate(boolean adapterInitialized, String selectedId, String nextId) {
        return adapterInitialized && !Objects.equals(selectedId, nextId);
    }

    static <C extends Component> C requireComponent(
        ParentComponent root,
        Class<C> expectedClass,
        String id
    ) {
        Objects.requireNonNull(root, "root must not be null");
        Objects.requireNonNull(expectedClass, "expectedClass must not be null");
        Objects.requireNonNull(id, "id must not be null");
        C component = root.childById(expectedClass, id);
        if (component == null) {
            throw new IllegalStateException("missing XML component id: " + id);
        }
        return component;
    }

    protected final LabelComponent label(String id) {
        return component(LabelComponent.class, id);
    }

    public final String templateIdForTests() {
        return templateId;
    }

    public final String selectedTemplateIdForTests() {
        return selectedTemplateId;
    }

    public final boolean scopeClosedForTests() {
        return lifecycle.isClosed();
    }

    public final boolean hostReadyForTests() {
        return uiAdapter != null && !invalid;
    }

    public final boolean hostInitializationFailedForTests() {
        return invalid;
    }

    public final ComponentBounds componentBoundsForPreview(String id) {
        Component value = component(Component.class, id);
        return new ComponentBounds(value.x(), value.y(), value.width(), value.height());
    }

    public final String componentIdAtForPreview(double logicalX, double logicalY) {
        if (uiAdapter == null) {
            throw new IllegalStateException("XML component tree is not initialized");
        }
        Component hit = uiAdapter.rootComponent.childAt((int) Math.floor(logicalX), (int) Math.floor(logicalY));
        return hit == null ? null : hit.id();
    }

    public final List<String> focusOrderForPreview() {
        if (uiAdapter == null) {
            throw new IllegalStateException("XML component tree is not initialized");
        }
        List<String> result = new ArrayList<>();
        uiAdapter.rootComponent.forEachDescendant(value -> {
            if (value.canFocus(Component.FocusSource.KEYBOARD_CYCLE)) {
                result.add(value.id() == null ? "<missing:" + value.getClass().getSimpleName() + ">" : value.id());
            }
        });
        return List.copyOf(result);
    }

    public record ComponentBounds(int x, int y, int width, int height) {
        public boolean isPositive() {
            return width > 0 && height > 0;
        }

        public boolean fitsInside(int viewportWidth, int viewportHeight) {
            return isPositive()
                && x >= 0
                && y >= 0
                && x + width <= viewportWidth
                && y + height <= viewportHeight;
        }

        public boolean contains(ComponentBounds other) {
            return other != null
                && other.isPositive()
                && other.x >= x
                && other.y >= y
                && other.x + other.width <= x + width
                && other.y + other.height <= y + height;
        }

        public double centerX() {
            return x + width / 2.0d;
        }

        public double centerY() {
            return y + height / 2.0d;
        }
    }
}
