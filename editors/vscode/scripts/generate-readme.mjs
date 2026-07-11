#!/usr/bin/env node
// Regenerates the extension's derived documentation from its definition files,
// so the documented snippets and examples cannot drift from what the extension ships.
//
// Outputs:
// - README.md "GENERATED: snippets" region: one section per snippet in
//   snippets/reportage.json, each embedding its runnable example examples/<id>.repor
// - examples/full-syntax.repor: concatenation of every per-snippet example, kept as the
//   all-in-one highlighting preview for the Extension Development Host
//
// Every example file runs in the project's e2e suite (reportage.examples.kdl globs
// editors/vscode/examples/*.repor), so a documented example is always working syntax.
//
// Usage:
//   node scripts/generate-readme.mjs           rewrite the outputs in place
//   node scripts/generate-readme.mjs --check   exit 1 if any output is stale (for CI)

import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const extensionRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const readmePath = join(extensionRoot, "README.md");
const examplesDir = join(extensionRoot, "examples");
const fullSyntaxName = "full-syntax.repor";
const fullSyntaxPath = join(examplesDir, fullSyntaxName);

// Snippet identity is the snake_case form of the snippet's JSON key (its VS Code
// display title). Renaming a title therefore requires renaming examples/<id>.repor
// in lockstep; the missing-example check turns a forgotten rename into a loud failure.
function idFor(title) {
  const id = title
    .toLowerCase()
    .replaceAll(/[^a-z0-9]+/g, "_")
    .replaceAll(/^_+|_+$/g, "");
  if (id === "") {
    throw new Error(`Snippet title ${JSON.stringify(title)} normalizes to an empty ID`);
  }
  return id;
}

function loadSnippets() {
  const raw = JSON.parse(readFileSync(join(extensionRoot, "snippets", "reportage.json"), "utf8"));
  const byId = new Map();
  for (const [title, snippet] of Object.entries(raw)) {
    const id = idFor(title);
    if (byId.has(id)) {
      throw new Error(
        `Snippet ID "${id}" collides between ${JSON.stringify(byId.get(id).title)} and ${JSON.stringify(title)}; rename one of them`,
      );
    }
    byId.set(id, { id, title, ...snippet });
  }
  return [...byId.values()];
}

// Enforces the 1:1 mapping in both directions:
// a snippet without an example and an example without a snippet both fail the build.
function loadExamples(entries) {
  const files = readdirSync(examplesDir).filter(
    (file) => file.endsWith(".repor") && file !== fullSyntaxName,
  );
  const expected = new Set(entries.map((entry) => `${entry.id}.repor`));
  const orphans = files.filter((file) => !expected.has(file));
  if (orphans.length > 0) {
    throw new Error(
      `Example files without a matching snippet: ${orphans.join(", ")}; remove them or add the corresponding snippet`,
    );
  }
  const examples = new Map();
  for (const entry of entries) {
    let content;
    try {
      content = readFileSync(join(examplesDir, `${entry.id}.repor`), "utf8");
    } catch {
      throw new Error(
        `Snippet ${JSON.stringify(entry.title)} has no example; create examples/${entry.id}.repor`,
      );
    }
    examples.set(entry.id, content.trimEnd());
  }
  return examples;
}

function formatPrefixes(prefix) {
  const prefixes = Array.isArray(prefix) ? prefix : [prefix];
  return prefixes.map((p) => `\`${p}\``).join(" / ");
}

// Fenced blocks must out-fence their content:
// reportage heredocs use ``` themselves, so a plain three-backtick fence would end early.
function fenceFor(text) {
  const longestRun = Math.max(0, ...[...text.matchAll(/`+/g)].map((m) => m[0].length));
  return "`".repeat(Math.max(4, longestRun + 1));
}

function renderSnippetsSection(entries, examples) {
  return entries
    .map((entry) => {
      const example = examples.get(entry.id);
      const fence = fenceFor(example);
      return [
        `### ${entry.title}`,
        `${formatPrefixes(entry.prefix)} — ${entry.description}`,
        [
          "<details>",
          "<summary>Example</summary>",
          "",
          `${fence}reportage`,
          example,
          fence,
          "",
          `From [examples/${entry.id}.repor](examples/${entry.id}.repor).`,
          "",
          "</details>",
        ].join("\n"),
      ].join("\n\n");
    })
    .join("\n\n");
}

const FULL_SYNTAX_HEADER = [
  "# GENERATED FILE - do not edit; run `pnpm run generate:readme`.",
  "# Concatenation of every per-snippet example in this directory, kept as a single",
  "# file so the Extension Development Host can preview highlighting for every construct.",
  "# Like the per-snippet files, it runs in the project's e2e suite (reportage.examples.kdl).",
].join("\n");

function renderFullSyntax(entries, examples) {
  // The concatenated file is one reportage test definition,
  // so case names must stay unique across all per-snippet examples.
  // The regex assumes the layout used throughout this directory: `case` at column 0
  // followed by a single space; indented or oddly spaced case lines would evade the check.
  const caseNames = new Map();
  for (const entry of entries) {
    for (const match of examples.get(entry.id).matchAll(/^case "((?:[^"\\]|\\.)*)"/gm)) {
      const owner = caseNames.get(match[1]);
      if (owner !== undefined) {
        throw new Error(
          `case ${JSON.stringify(match[1])} appears in both ${owner} and ${entry.id}.repor; case names must be unique for the concatenated ${fullSyntaxName}`,
        );
      }
      caseNames.set(match[1], `${entry.id}.repor`);
    }
  }
  const parts = [FULL_SYNTAX_HEADER, ...entries.map((entry) => examples.get(entry.id))];
  return `${parts.join("\n\n")}\n`;
}

function replaceRegion(readme, name, content) {
  const begin = `<!-- BEGIN GENERATED: ${name} -->`;
  const end = `<!-- END GENERATED: ${name} -->`;
  const beginIndex = readme.indexOf(begin);
  const endIndex = readme.indexOf(end);
  if (beginIndex === -1 || endIndex === -1 || endIndex < beginIndex) {
    throw new Error(`README.md is missing a well-formed "${name}" generated region`);
  }
  const note = "<!-- Generated by scripts/generate-readme.mjs; edit the sources, not this region. -->";
  return `${readme.slice(0, beginIndex + begin.length)}\n${note}\n\n${content}\n\n${readme.slice(endIndex)}`;
}

function readIfExists(path) {
  try {
    return readFileSync(path, "utf8");
  } catch {
    return "";
  }
}

function main() {
  const checkMode = process.argv.includes("--check");

  const entries = loadSnippets();
  const examples = loadExamples(entries);

  const outputs = [
    {
      label: "README.md",
      path: readmePath,
      current: readFileSync(readmePath, "utf8"),
      next: replaceRegion(readFileSync(readmePath, "utf8"), "snippets", renderSnippetsSection(entries, examples)),
    },
    {
      label: `examples/${fullSyntaxName}`,
      path: fullSyntaxPath,
      current: readIfExists(fullSyntaxPath),
      next: renderFullSyntax(entries, examples),
    },
  ];

  const stale = outputs.filter((output) => output.next !== output.current);

  if (checkMode) {
    if (stale.length > 0) {
      console.error(
        `Stale generated files: ${stale.map((o) => o.label).join(", ")}; run \`pnpm run generate:readme\` and commit the result.`,
      );
      process.exit(1);
    }
    console.log("Generated files are up to date.");
    return;
  }

  for (const output of stale) {
    writeFileSync(output.path, output.next);
    console.log(`${output.label} regenerated.`);
  }
  if (stale.length === 0) {
    console.log("Generated files already up to date.");
  }
}

main();
