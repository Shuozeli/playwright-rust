import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

import ts from "../packages/playwright-rust/node_modules/typescript/lib/typescript.js";

// Last Test262 main commit before Playwright v1.58.0 was tagged at
// 2026-01-23T10:37:04Z.
const EXPECTED_COMMIT = "3c6180f581e5f468d6767af2f9b1f9c3ca444e77";
const [test262Argument, outputArgument] = process.argv.slice(2);
if (!test262Argument || !outputArgument) {
  throw new Error(
    "usage: node scripts/generate_test262_regexp_corpus.mjs <test262-root> <output-json>",
  );
}

const test262Root = resolve(test262Argument);
const regexpRoot = resolve(test262Root, "test/built-ins/RegExp");
const output = resolve(outputArgument);
const oracleScript = fileURLToPath(new URL("./test262_regexp_oracle.mjs", import.meta.url));
const commit = execFileSync("git", ["-C", test262Root, "rev-parse", "HEAD"], {
  encoding: "utf8",
}).trim();
if (commit !== EXPECTED_COMMIT) {
  throw new Error(`Test262 must be checked out at ${EXPECTED_COMMIT}; found ${commit}`);
}

const genericInputs = [
  "",
  "a",
  "A",
  "0",
  " ",
  "\n",
  "\r\n",
  "abc",
  "123",
  "https://example.test/api/v2?item=1",
  "😀",
  "\ud800",
];

function walk(directory) {
  const files = [];
  for (const name of readdirSync(directory).sort()) {
    const path = resolve(directory, name);
    if (statSync(path).isDirectory()) files.push(...walk(path));
    else if (name.endsWith(".js")) files.push(path);
  }
  return files;
}

function parseLiteral(text) {
  const separator = text.lastIndexOf("/");
  if (!text.startsWith("/") || separator === 0) return undefined;
  return { source: text.slice(1, separator), flags: text.slice(separator + 1) };
}

function staticString(node) {
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) {
    return node.text;
  }
  return undefined;
}

function utf16leBase64(value) {
  const bytes = Buffer.allocUnsafe(value.length * 2);
  for (let index = 0; index < value.length; index += 1) {
    bytes.writeUInt16LE(value.charCodeAt(index), index * 2);
  }
  return bytes.toString("base64");
}

function normalizeUnpairedSurrogates(value) {
  let normalized = "";
  let changed = false;
  for (let index = 0; index < value.length; index += 1) {
    const unit = value.charCodeAt(index);
    const next = index + 1 < value.length ? value.charCodeAt(index + 1) : undefined;
    if (unit >= 0xd800 && unit <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) {
      normalized += value[index] + value[index + 1];
      index += 1;
    } else if (unit >= 0xd800 && unit <= 0xdfff) {
      normalized += `\\u${unit.toString(16).padStart(4, "0")}`;
      changed = true;
    } else {
      normalized += value[index];
    }
  }
  return { normalized, changed };
}

function extract(path) {
  const text = readFileSync(path, "utf8");
  const sourceFile = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true);
  const patterns = new Map();
  const strings = [];
  const visit = (node) => {
    if (ts.isRegularExpressionLiteral(node)) {
      const pattern = parseLiteral(node.text);
      if (pattern) patterns.set(`${pattern.source}\u0000${pattern.flags}`, pattern);
    }
    if (
      ts.isNewExpression(node) &&
      ts.isIdentifier(node.expression) &&
      node.expression.text === "RegExp"
    ) {
      const args = node.arguments ?? [];
      const source = args[0] ? staticString(args[0]) : "";
      const flags = args[1] ? staticString(args[1]) : "";
      if (source !== undefined && flags !== undefined) {
        patterns.set(`${source}\u0000${flags}`, { source, flags });
      }
    }
    const value = staticString(node);
    if (value !== undefined && value.length <= 1024) strings.push(value);
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);

  const probes = [...new Set([...genericInputs, ...strings])].slice(0, 24);
  const extractedPatterns = [...patterns.values()];
  const oracleResults = JSON.parse(
    execFileSync(process.execPath, [oracleScript], {
      input: JSON.stringify({ patterns: extractedPatterns, probes }),
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
    }),
  );
  return extractedPatterns.map(({ source, flags }, patternIndex) => {
    const originalSource = source;
    const normalizedSource = normalizeUnpairedSurrogates(source);
    const oracle = oracleResults[patternIndex];
    return {
      source: normalizedSource.normalized,
      flags,
      normalizedUnpairedSource: normalizedSource.changed,
      expectedCompile: oracle.expectedCompile,
      steps: oracle.steps.map(({ inputIndex, matched, lastIndex }) => ({
        inputUtf16leBase64: utf16leBase64(probes[inputIndex]),
        matched,
        lastIndex,
      })),
    };
  });
}

const cases = [];
for (const path of walk(regexpRoot)) {
  const file = relative(test262Root, path);
  for (const pattern of extract(path)) cases.push({ file, ...pattern });
}
cases.sort((left, right) =>
  left.file.localeCompare(right.file) ||
  left.source.localeCompare(right.source) ||
  left.flags.localeCompare(right.flags),
);

const corpus = {
  test262Commit: commit,
  nodeVersion: process.version,
  sourceDirectory: "test/built-ins/RegExp",
  caseCount: cases.length,
  normalizedUnpairedSourceCount: cases.filter((entry) => entry.normalizedUnpairedSource).length,
  cases: cases.map(({ normalizedUnpairedSource: _, ...entry }) => entry),
};
const json = Buffer.from(`${JSON.stringify(corpus, null, 2)}\n`);
const contents = output.endsWith(".gz") ? gzipSync(json, { level: 9, mtime: 0 }) : json;
writeFileSync(output, contents);
console.log(`wrote ${cases.length} cases to ${output}`);
