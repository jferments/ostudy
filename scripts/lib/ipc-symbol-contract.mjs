// Tauri 2 generates its exported helper macros from Rust function identifiers.
// Wire aliases are separately unique among registered (not all defined) commands.
export function duplicateFunctionNames(definitionsByFunction) {
  return [...definitionsByFunction].filter(([, definitions]) => definitions.length > 1);
}
