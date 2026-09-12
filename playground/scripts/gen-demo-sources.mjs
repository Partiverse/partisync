/**
 * gen-demo-sources.mjs — 构建时源码提取生成器
 *
 * 解析 src/showcase/{basics,forms,overlays,navigation,data}.tsx，
 * 对每个 <Demo title="T"> 元素：
 *  - 默认提取 children 的源码原文（AST 节点 getFullStart→getEnd 切原始文本，去首尾空行）
 *  - 若 children 是单个组件调用 <X />（或 <X>…</X> 无内容）且 X 是同文件内的
 *    function/const 组件定义，改用该定义的完整源码文本
 *
 * 输出 src/demo-sources.generated.ts：
 *   export const demoSources: Record<string, string> = { "文件基名#标题": "源码", ... }
 *
 * 同文件同名 title 冲突时报错退出非零。
 */
import fs from "node:fs";
import path from "node:path";
import url from "node:url";
import ts from "typescript";

const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
const playgroundRoot = path.resolve(__dirname, "..");
const showcaseDir = path.join(playgroundRoot, "src", "showcase");
const outFile = path.join(playgroundRoot, "src", "demo-sources.generated.ts");

const TARGET_FILES = ["basics", "forms", "overlays", "navigation", "data"];

/** 去掉首尾空行并剥离最小公共前导缩进（dedent） */
function trimBlankEdges(text) {
  // 去除首尾空行
  const trimmed = text
    .replace(/^\s*\n/, "")
    .replace(/\n\s*$/, "")
    .replace(/\n[ \t]+$/, "\n");

  const lines = trimmed.split("\n");
  const nonBlankLines = lines.filter((line) => line.trim().length > 0);
  if (nonBlankLines.length === 0) {
    return "";
  }

  // 计算所有非空行的最小公共前导空白
  let minIndent = Infinity;
  for (const line of nonBlankLines) {
    const match = line.match(/^[ \t]*/);
    const indent = match ? match[0].length : 0;
    if (indent < minIndent) {
      minIndent = indent;
    }
  }

  // 剥离公共缩进，空行置为空字符串
  return lines
    .map((line) => {
      if (line.trim().length === 0) {
        return "";
      }
      return line.slice(minIndent);
    })
    .join("\n");
}

/** 收集同文件顶层组件定义：function X(){...} 与 const X = (...) => ... / function 表达式 */
function collectComponentDefs(sourceFile) {
  const defs = new Map(); // name -> { start, end }
  for (const st of sourceFile.statements) {
    if (ts.isFunctionDeclaration(st) && st.name && !st.modifiers?.some((m) => m.kind === ts.SyntaxKind.ExportKeyword && false)) {
      // export default function XxxSection 也可能是定义，但不会被 <X /> 引用为子元素以外场景，无碍
      defs.set(st.name.text, { start: st.getStart(), end: st.getEnd() });
      continue;
    }
    if (ts.isVariableStatement(st)) {
      for (const decl of st.declarationList.declarations) {
        if (!ts.isIdentifier(decl.name)) continue;
        const init = decl.initializer;
        if (
          init &&
          (ts.isArrowFunction(init) ||
            ts.isFunctionExpression(init))
        ) {
          // const X = ... 的定义文本取整个声明（含类型标注），比仅取 init 更完整
          defs.set(decl.name.text, { start: st.getStart(), end: st.getEnd() });
        }
      }
    }
  }
  return defs;
}

/** 从 JSX 元素取 title 属性值 */
function getTitleAttr(el) {
  if (!ts.isJsxOpeningElement(el.openingElement)) return undefined;
  for (const attr of el.openingElement.attributes.properties) {
    if (
      ts.isJsxAttribute(attr) &&
      ts.isIdentifier(attr.name) &&
      attr.name.text === "title" &&
      attr.initializer &&
      ts.isStringLiteral(attr.initializer)
    ) {
      return attr.initializer.text;
    }
  }
  return undefined;
}

/** 判断 children 是否为「单个组件调用 <X />」，返回组件名或 null */
function singleComponentChild(children) {
  const elements = children.filter((c) => ts.isJsxElement(c) || ts.isJsxSelfClosingElement(c));
  const others = children.filter((c) => !ts.isJsxElement(c) && !ts.isJsxSelfClosingElement(c));
  // 允许夹带纯空白 JSXText
  const hasNonWhitespaceText = others.some((c) => !(ts.isJsxText(c) && c.text.trim() === ""));
  if (elements.length !== 1 || hasNonWhitespaceText) return null;
  const el = elements[0];
  const tagName = ts.isJsxSelfClosingElement(el)
    ? el.tagName
    : el.openingElement.tagName;
  if (!ts.isIdentifier(tagName)) return null;
  // 自闭合或无子内容的闭合标签才算“组件调用”
  if (ts.isJsxElement(el) && el.children.some((c) => !(ts.isJsxText(c) && c.text.trim() === ""))) {
    return null;
  }
  return tagName.text;
}

function processFile(filePath, sources, errors, category) {
  const base = category || path.basename(filePath, ".tsx");
  const raw = fs.readFileSync(filePath, "utf8");
  const sourceFile = ts.createSourceFile(filePath, raw, ts.ScriptTarget.Latest, /*setParentNodes*/ true);
  const defs = collectComponentDefs(sourceFile);

  const seenTitles = new Map(); // title -> line
  function visit(node) {
    if (ts.isJsxElement(node) || ts.isJsxSelfClosingElement(node)) {
      const el = node;
      const tagName = ts.isJsxSelfClosingElement(el)
        ? el.tagName
        : el.openingElement.tagName;
      if (ts.isIdentifier(tagName) && tagName.text === "Demo") {
        const title = getTitleAttr(el);
        if (title === undefined) {
          errors.push(`${path.basename(filePath)}: <Demo> 缺少 title 字符串属性`);
        } else {
          const line = sourceFile.getLineAndCharacterOfPosition(el.getStart()).line + 1;
          const fullKey = `${base}#${title}`;
          if (sources.has(fullKey)) {
            errors.push(
              `${base} (${path.basename(filePath)}): title "${title}" 重复出现`
            );
          } else {
            seenTitles.set(title, line);
          }

          let code;
          const compName = ts.isJsxElement(el)
            ? singleComponentChild(el.children)
            : ts.isIdentifier(el.tagName)
              ? el.tagName.text
              : null;
          if (compName && defs.has(compName)) {
            // stateful：取同文件定义的完整源码
            const def = defs.get(compName);
            code = trimBlankEdges(raw.slice(def.start, def.end));
          } else if (ts.isJsxElement(el)) {
            // children 原文：首子节点 getFullStart（含前置空白）→ 末子节点 getEnd
            const kids = el.children;
            const start = kids[0].getFullStart();
            const end = kids[kids.length - 1].getEnd();
            code = trimBlankEdges(raw.slice(start, end));
          } else {
            code = "";
          }

          sources.set(fullKey, code);
        }
      }
    }
    ts.forEachChild(node, visit);
  }
  visit(sourceFile);
}

/** 生成器主入口（可被 vite.config.ts 以 import 方式复用） */
export function runGenerator() {
  const sources = new Map();
  const errors = [];

  for (const category of TARGET_FILES) {
    const dirPath = path.join(showcaseDir, category);
    const singleFilePath = path.join(showcaseDir, `${category}.tsx`);

    if (fs.existsSync(dirPath) && fs.statSync(dirPath).isDirectory()) {
      // 遍历扫描子目录下的所有 .tsx 文件（排除 index.tsx）
      const entries = fs
        .readdirSync(dirPath, { withFileTypes: true })
        .filter((ent) => ent.isFile() && ent.name.endsWith(".tsx") && ent.name !== "index.tsx")
        .sort((a, b) => a.name.localeCompare(b.name));

      for (const entry of entries) {
        const subFilePath = path.join(dirPath, entry.name);
        processFile(subFilePath, sources, errors, category);
      }
    } else if (fs.existsSync(singleFilePath)) {
      processFile(singleFilePath, sources, errors, category);
    } else {
      errors.push(`缺少目标模块: ${category}`);
    }
  }

  if (errors.length) {
    console.error("gen-demo-sources: 生成失败");
    for (const e of errors) console.error("  - " + e);
    process.exit(1);
  }

  // JSON.stringify 防注入；按文件分组排序保持稳定输出
  const entries = [...sources.entries()].sort(([a], [b]) => {
    const [fa, ta] = a.split("#");
    const [fb, tb] = b.split("#");
    return fa === fb ? ta.localeCompare(tb) : fa.localeCompare(fb);
  });
  const body = entries.map(([k, v]) => `  ${JSON.stringify(k)}: ${JSON.stringify(v)}`).join(",\n");

  const content = `// 本文件由 scripts/gen-demo-sources.mjs 构建时自动生成，请勿手写。\n// 源码面板数据：键为 "文件基名#Demo标题"，值为该 Demo 渲染代码的源码原文。\nexport const demoSources: Record<string, string> = {\n${body},\n}\n`;

  const prev = fs.existsSync(outFile) ? fs.readFileSync(outFile, "utf8") : "";
  fs.writeFileSync(outFile, content);
  console.log(
    `gen-demo-sources: ${entries.length} 个条目 → ${path.relative(playgroundRoot, outFile)}${prev === content ? " (无变化)" : ""}`
  );
}

// 直接执行时（node scripts/gen-demo-sources.mjs）运行；被 import 时只导出 runGenerator
if (process.argv[1] && import.meta.url === url.pathToFileURL(process.argv[1]).href) {
  runGenerator();
}
