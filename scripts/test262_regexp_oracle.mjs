import { readFileSync } from "node:fs";

const { patterns, probes } = JSON.parse(readFileSync(0, "utf8"));
const results = patterns.map(({ source, flags }) => {
  let regexp;
  try {
    regexp = new RegExp(source, flags);
  } catch {
    return { expectedCompile: false, steps: [] };
  }
  return {
    expectedCompile: true,
    steps: probes.map((input, inputIndex) => ({
      inputIndex,
      matched: regexp.test(input),
      lastIndex: regexp.lastIndex,
    })),
  };
});
process.stdout.write(JSON.stringify(results));
