from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}")
    target.write_text(text.replace(old, new, 1), encoding="utf-8")


replace_once(
    "src/screens/settings/AiLlmSettings.tsx",
    '''            {controller.catalog.map((entry) => (
              <option
                key={entry.providerKind}
                value={entry.providerKind}
                disabled={entry.implementationStatus !== "available"}
              >
                {entry.displayName}
                {entry.implementationStatus === "available" ? "" : " — planned"}
              </option>
            ))}
''',
    '''            {controller.catalog
              .filter((entry) => entry.implementationStatus === "available")
              .map((entry) => (
                <option key={entry.providerKind} value={entry.providerKind}>
                  {entry.displayName}
                </option>
              ))}
''',
)

replace_once(
    "src/screens/SettingsScreen.test.tsx",
    '''    expect(screen.queryByRole("heading", { name: "Planned providers" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "New profile" }));
''',
    '''    expect(screen.queryByRole("heading", { name: "Planned providers" })).toBeNull();
    expect(screen.queryByRole("option", { name: /planned/u })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "New profile" }));
''',
)

print("planned providers removed from AI settings UI")
