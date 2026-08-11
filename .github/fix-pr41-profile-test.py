from pathlib import Path

path = Path("src/screens/SettingsScreen.extended.test.tsx")
text = path.read_text(encoding="utf-8")
old = '    fireEvent.click(screen.getByRole("button", { name: "Add llama.cpp" }));\n'
new = '''    fireEvent.click(screen.getByRole("button", { name: "New profile" }));
    expect(
      (screen.getByRole("combobox", { name: "Provider type" }) as HTMLSelectElement).value,
    ).toBe("llama_cpp");
'''
count = text.count(old)
if count != 1:
    raise SystemExit(f"expected one legacy Add llama.cpp test action, found {count}")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
