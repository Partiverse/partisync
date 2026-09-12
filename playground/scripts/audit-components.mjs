import fs from "fs";
import path from "path";

const dir = "src/components/ui";
const files = fs.readdirSync(dir).filter(f => f.endsWith(".tsx"));
console.log(`Auditing ${files.length} UI components...`);

const results = [];

for (const file of files) {
  const code = fs.readFileSync(path.join(dir, file), "utf8");
  const comp = file.replace(".tsx", "");

  const usesBaseUi = code.includes("@base-ui/react");
  const usesRadix = code.includes("@radix-ui");
  const usesCmdk = code.includes("cmdk");
  const usesRecharts = code.includes("recharts");
  const usesEmbla = code.includes("embla");
  const usesInputOtp = code.includes("input-otp");
  const usesSonner = code.includes("sonner");
  const usesDayPicker = code.includes("react-day-picker");

  // Check render prop usage
  const renderProps = [];
  const renderMatches = [...code.matchAll(/render=\{([^}]+)\}/g)];
  for (const m of renderMatches) {
    renderProps.push(m[1].trim());
  }

  // Check missing official exports/props if official exists
  let officialDiff = null;
  const offPath = `/tmp/base-nova/${comp}.official.tsx`;
  if (fs.existsSync(offPath)) {
    const offCode = fs.readFileSync(offPath, "utf8");
    // check exports
    const localExports = [...code.matchAll(/export\s*\{([^}]+)\}/g)].flatMap(m => m[1].split(",").map(x => x.trim().split(" ")[0])).filter(Boolean);
    const offExports = [...offCode.matchAll(/export\s*\{([^}]+)\}/g)].flatMap(m => m[1].split(",").map(x => x.trim().split(" ")[0])).filter(Boolean);
    const missingExports = offExports.filter(e => !localExports.includes(e));
    const extraExports = localExports.filter(e => !offExports.includes(e));

    officialDiff = { missingExports, extraExports };
  }

  results.push({
    file,
    comp,
    primitive: usesBaseUi ? "base-ui" : usesRadix ? "radix-ui" : usesCmdk ? "cmdk" : usesRecharts ? "recharts" : usesEmbla ? "embla" : usesSonner ? "sonner" : usesDayPicker ? "react-day-picker" : "plain/native",
    renderProps,
    officialDiff
  });
}

console.log(JSON.stringify(results, null, 2));
