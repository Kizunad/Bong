/**
 * 将已完成的植物作者模型导出为客户端 geo + PNG，不重跑生成器、不覆盖作者稿。
 * 几何和旋转沿用现有 Blockbench Bedrock codec；植物以方块底面中心为锚点。
 * 用法：node modelScript/exporters/export_plant_assets.cjs [--check]
 */
const fs = require("node:fs");
const path = require("node:path");
const { convert, formatJson } = require("./creature_codec.cjs");

const ROOT = path.resolve(__dirname, "../..");
const MODELS = path.join(ROOT, "modelScript/models");
const ASSETS = path.join(ROOT, "client/src/main/resources/assets/bong");
const PLANTS = {
    chi_sui_cao: "ChiSuiCao",
    gu_yuan_gen: "GuYuanGen",
    hei_gu_jun: "HeiGuJun",
    ling_yan_shi_zhi: "LingYanShiZhi",
    xue_po_lian: "XuePoLian",
    tui_gu_teng: "TuiGuTeng",
    shou_xin_cao: "ShouXinCao",
    long_lin_tai: "LongLinTai",
    xu_yuan_rui: "XuYuanRui",
    hua_xing_gen: "HuaXingGen",
    hui_jin_tai: "HuiJinTai",
    lie_yuan_tai: "LieYuanTai",
    shi_ling_xian: "ShiLingXian",
    xuan_rong_tai: "XuanRongTai",
    yang_jing_tai: "YangJingTai",
    jing_xin_zao: "JingXinZao",
    bei_wen_zhi: "BeiWenZhi",
    ming_gu_gu: "MingGuGu",
    ying_yuan_gu: "YingYuanGu",
    kong_shou_hen: "KongShouHen",
    ling_jing_xu: "LingJingXu",
    bai_yan_peng: "BaiYanPeng",
    jiao_mai_teng: "JiaoMaiTeng",
    ye_ku_teng: "YeKuTeng",
    zhong_yan_teng: "ZhongYanTeng",
    shao_hou_man: "ShaoHouMan",
    xuan_gen_wei: "XuanGenWei",
    hui_yuan_zhi: "HuiYuanZhi",
};

function centerModel(source) {
    const model = structuredClone(source);
    // 这批作者稿使用 0..16 的方块坐标，底面中心为 [8, 0, 8]。
    // 同时移动几何和旋转轴，避免花瓣/藤蔓围绕旧轴偏移。
    function move(point) {
        if (point) {
            point[0] -= 8;
            point[2] -= 8;
        }
    }
    for (const element of model.elements) {
        move(element.from);
        move(element.to);
        move(element.origin);
    }
    function moveGroup(group) {
        if (typeof group === "string") return;
        move(group.origin);
        for (const child of group.children || []) moveGroup(child);
    }
    for (const group of model.outliner || []) moveGroup(group);
    for (const group of model.groups || []) move(group.origin);
    return model;
}

function buildPlant(source, id) {
    if ((source.animations || []).length) {
        throw new Error(`${id}: 作者稿新增了动画，请先补齐动画导出契约`);
    }
    if (source.textures?.length !== 1 || !source.textures[0].source?.startsWith("data:image/png;base64,")) {
        throw new Error(`${id}: 必须有一张内嵌 PNG 图集`);
    }
    const texture = Buffer.from(source.textures[0].source.split(",")[1], "base64");
    if (texture.length < 24 || texture.subarray(0, 8).toString("hex") !== "89504e470d0a1a0a") {
        throw new Error(`${id}: PNG 图集损坏`);
    }
    const { geometry } = convert(centerModel(source), id);
    const model = geometry["minecraft:geometry"][0];
    const cubes = model.bones.reduce((count, bone) => count + (bone.cubes || []).length, 0);
    if (cubes !== source.elements.length) throw new Error(`${id}: 导出丢失几何`);
    if (model.description.texture_width !== texture.readUInt32BE(16)
        || model.description.texture_height !== texture.readUInt32BE(20)) {
        throw new Error(`${id}: 图集分辨率与模型 UV 尺寸不一致`);
    }
    return { geometry: Buffer.from(formatJson(geometry)), texture };
}

function exportPlants(check = false, outputRoot = ASSETS) {
    // 全部转换成功后才安装，避免一半资源更新、一半仍是旧资源。
    const outputs = [];
    for (const [id, name] of Object.entries(PLANTS)) {
        const source = JSON.parse(fs.readFileSync(path.join(MODELS, `${name}.bbmodel`), "utf8"));
        const result = buildPlant(source, id);
        outputs.push([path.join(outputRoot, "geo/plants", `${id}.geo.json`), result.geometry]);
        outputs.push([path.join(outputRoot, "textures/entity/plants", `${id}.png`), result.texture]);
    }
    for (const [target, bytes] of outputs) {
        if (check) {
            if (!fs.existsSync(target) || !fs.readFileSync(target).equals(bytes)) {
                throw new Error(`植物运行时资源与作者稿不一致：${target}`);
            }
        } else {
            fs.mkdirSync(path.dirname(target), { recursive: true });
            fs.writeFileSync(target, bytes);
        }
    }
    return Object.keys(PLANTS).length;
}

if (require.main === module) {
    if (process.argv.slice(2).some(arg => arg !== "--check")) throw new Error("仅支持 --check");
    console.log(`植物模型 ${process.argv.includes("--check") ? "校验" : "导出"}完成：${exportPlants(process.argv.includes("--check"))} 种`);
}

module.exports = { buildPlant, centerModel, exportPlants, PLANTS };
