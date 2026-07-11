#!/usr/bin/env node
// Regenerates the marked regions of README.md from the extension's own definition files,
// so the documented snippets and examples cannot drift from what the extension ships.
//
// Sources:
// - snippets/reportage.json -> "GENERATED: snippets" region
// - examples/full-syntax.repor -> "GENERATED: example" region (kept passing by the project's e2e suite)
//
// Usage:
//   node scripts/generate-readme.mjs           rewrite README.md in place
//   node scripts/generate-readme.mjs --check   exit 1 if README.md is stale (for CI)

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const extensionRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const readmePath = join(extensionRoot, "README.md");

// Category assignment for the snippet listing.
// Snippet JSON has no category field, so the mapping lives here;
// a snippet whose prefix matches no entry fails the build instead of silently disappearing.
const CATEGORIES = [
  { title: "Case Structure", prefixes: ["case"] },
  { title: "Action Steps", prefixes: ["$", "action"] },
  { title: "Write Steps", prefixes: ["write", "writeh"] },
  { title: "Assertion Blocks", prefixes: ["assert", "assertb"] },
  { title: "Exit Code Assertions", prefixes: ["exit"] },
  { title: "Stdout Assertions", pattern: /^stdout-/ },
  { title: "Stderr Assertions", pattern: /^stderr-/ },
  { title: "File Assertions", pattern: /^file-/ },
  { title: "Directory Assertions", pattern: /^dir-/ },
  { title: "Operators", prefixes: ["not", "all", "any"] },
  { title: "Literals", prefixes: ["wpath", "fixture"] },
];

// Cases pulled out of examples/full-syntax.repor into the README, in display order.
const EXAMPLE_CASES = [
  "actions and write steps",
  "expectations across all kinds and literal forms",
];

function matchCategory(prefixes) {
  return CATEGORIES.find(
    (category) =>
      prefixes.some((prefix) => category.prefixes?.includes(prefix)) ||
      prefixes.some((prefix) => category.pattern?.test(prefix)),
  );
}

// Turns a snippet body into the text VS Code inserts, minus editing artifacts:
// placeholders keep their default text, bare tabstops disappear, tabs become two spaces.
// Only the constructs used by snippets/reportage.json are supported;
// anything else (choices, variables, nested placeholders) fails loudly
// so it cannot pass through garbled into the shipped README.
function expansionLines(body) {
  const lines = Array.isArray(body) ? body : [body];
  return lines.map((line) => {
    const stripped = line
      .replaceAll(/\$\{\d+:([^}]*)\}/g, "$1")
      .replaceAll(/\$\{\d+\}/g, "")
      .replaceAll(/\$\d+/g, "")
      .replaceAll("\t", "  ")
      .trimEnd();
    if (/\$\{|\$[A-Za-z_]/.test(stripped)) {
      throw new Error(
        `Unsupported snippet construct in body line ${JSON.stringify(line)}; teach expansionLines in scripts/generate-readme.mjs how to render it`,
      );
    }
    return stripped;
  });
}

function escapeTableCell(text) {
  return text.replaceAll("|", "\\|");
}

function formatPrefixes(prefix) {
  const prefixes = Array.isArray(prefix) ? prefix : [prefix];
  return prefixes.map((p) => `\`${p}\``).join(" / ");
}

function longestBacktickRun(text) {
  return Math.max(0, ...[...text.matchAll(/`+/g)].map((m) => m[0].length));
}

// Fenced blocks must out-fence their content:
// reportage heredocs use ``` themselves, so a plain three-backtick fence would end early.
function fenceFor(text) {
  return "`".repeat(Math.max(4, longestBacktickRun(text) + 1));
}

// Inline code spans must out-tick their content for the same reason fenceFor exists.
function inlineCode(text) {
  const run = longestBacktickRun(text);
  if (run === 0) return `\`${text}\``;
  const ticks = "`".repeat(run + 1);
  return `${ticks} ${text} ${ticks}`;
}

function renderSnippetsSection(snippets) {
  const grouped = new Map(CATEGORIES.map((category) => [category.title, []]));
  for (const [name, snippet] of Object.entries(snippets)) {
    const prefixes = Array.isArray(snippet.prefix) ? snippet.prefix : [snippet.prefix];
    const category = matchCategory(prefixes);
    if (!category) {
      throw new Error(
        `Snippet "${name}" (prefix: ${prefixes.join(", ")}) matches no category; add it to CATEGORIES in scripts/generate-readme.mjs`,
      );
    }
    grouped.get(category.title).push(snippet);
  }

  const sections = [];
  for (const category of CATEGORIES) {
    const entries = grouped.get(category.title);
    if (entries.length === 0) continue;

    const singleLine = entries.filter((snippet) => expansionLines(snippet.body).length === 1);
    const multiLine = entries.filter((snippet) => expansionLines(snippet.body).length > 1);

    const parts = [`### ${category.title}`];

    if (singleLine.length > 0) {
      const rows = singleLine.map((snippet) => {
        const expansion = expansionLines(snippet.body)[0];
        return `| ${formatPrefixes(snippet.prefix)} | ${escapeTableCell(inlineCode(expansion))} | ${escapeTableCell(snippet.description)} |`;
      });
      parts.push(["| Prefix | Expands to | Description |", "| --- | --- | --- |", ...rows].join("\n"));
    }

    for (const snippet of multiLine) {
      const expansion = expansionLines(snippet.body).join("\n");
      const fence = fenceFor(expansion);
      const indented = expansion
        .split("\n")
        .map((line) => (line === "" ? "" : `  ${line}`))
        .join("\n");
      parts.push(`- ${formatPrefixes(snippet.prefix)} — ${snippet.description}\n\n  ${fence}\n${indented}\n  ${fence}`);
    }

    sections.push(parts.join("\n\n"));
  }
  return sections.join("\n\n");
}

// Finds the end of the case by brace depth, skipping heredoc bodies so that
// free-form heredoc text (which may contain braces at any column) cannot truncate
// the extraction. Braces inside quoted strings are still counted; that is acceptable
// for this curated example file because a miscount fails loudly here rather than
// publishing a broken example.
function extractCase(source, name) {
  const lines = source.split("\n");
  const start = lines.findIndex((line) => line.startsWith(`case "${name}" {`));
  if (start === -1) {
    throw new Error(`case "${name}" not found in examples/full-syntax.repor`);
  }
  let depth = 0;
  let inHeredoc = false;
  for (let i = start; i < lines.length; i++) {
    const line = lines[i];
    if (inHeredoc) {
      if (/^\s*`{3,}\s*$/.test(line)) inHeredoc = false;
      continue;
    }
    if (/`{3,}\s*$/.test(line)) {
      inHeredoc = true;
      continue;
    }
    depth += (line.match(/\{/g) ?? []).length;
    depth -= (line.match(/\}/g) ?? []).length;
    if (depth < 0) {
      throw new Error(`case "${name}": unbalanced braces at line ${i + 1} of examples/full-syntax.repor`);
    }
    if (depth === 0) {
      return lines.slice(start, i + 1).join("\n");
    }
  }
  throw new Error(`case "${name}" is never closed in examples/full-syntax.repor`);
}

function renderExampleSection(exampleSource) {
  const intro =
    "The examples below are taken from [examples/full-syntax.repor](examples/full-syntax.repor), which is run by reportage's own e2e suite, so they always reflect working syntax.";
  const blocks = EXAMPLE_CASES.map((name) => {
    const body = extractCase(exampleSource, name);
    const fence = fenceFor(body);
    return `${fence}reportage\n${body}\n${fence}`;
  });
  return [intro, ...blocks].join("\n\n");
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

function main() {
  const checkMode = process.argv.includes("--check");

  const snippets = JSON.parse(readFileSync(join(extensionRoot, "snippets", "reportage.json"), "utf8"));
  const exampleSource = readFileSync(join(extensionRoot, "examples", "full-syntax.repor"), "utf8");
  const current = readFileSync(readmePath, "utf8");

  let next = current;
  next = replaceRegion(next, "snippets", renderSnippetsSection(snippets));
  next = replaceRegion(next, "example", renderExampleSection(exampleSource));

  if (checkMode) {
    if (next !== current) {
      console.error("README.md is out of date; run `pnpm run generate:readme` and commit the result.");
      process.exit(1);
    }
    console.log("README.md is up to date.");
    return;
  }

  if (next !== current) {
    writeFileSync(readmePath, next);
    console.log("README.md regenerated.");
  } else {
    console.log("README.md already up to date.");
  }
}

main();
