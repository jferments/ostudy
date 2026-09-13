import { posix } from "node:path";

const vendorRoot = "src/ui/vendor/shadcn/";
const adapterRoots = ["src/ui/primitives/", "src/ui/overlays/", "src/ui/layout/"];

/** Paths are repo-relative POSIX paths; imports may be relative or @/ aliases. */
export function importViolation(owner, specifier) {
  const source = owner.replaceAll("\\", "/");
  const target = specifier.startsWith("@/")
    ? posix.normalize(`src/${specifier.slice(2)}`)
    : specifier.startsWith(".")
      ? posix.normalize(posix.join(posix.dirname(source), specifier))
      : specifier;
  const vendorOwner = source.startsWith(vendorRoot);
  if (specifier === "@base-ui/react" || specifier.startsWith("@base-ui/react/")) {
    return vendorOwner ? null : "Base UI belongs inside src/ui/vendor/shadcn; import an oStudy facade.";
  }
  if ((target === vendorRoot.slice(0, -1) || target.startsWith(vendorRoot))
      && !vendorOwner && !adapterRoots.some((path) => source.startsWith(path))) {
    return "Only oStudy primitive/overlay/layout adapters may import shadcn implementation files.";
  }
  return null;
}
