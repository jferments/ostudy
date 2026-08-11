from pathlib import Path

path = Path("src/screens/PracticeScreen.tagScope.test.tsx")
text = path.read_text(encoding="utf-8")
old = '''          cardIds: [],
          autoStart: true,
        }}
        onLaunchConsumed={onLaunchConsumed}
        onViewAnalytics={vi.fn()}
        showToast={vi.fn()}
'''
new = '''          cardIds: [],
          autoStart: true,
          mode: "scheduled",
          returnTarget: { section: "analytics" },
        }}
        onLaunchConsumed={onLaunchConsumed}
        onViewAnalytics={vi.fn()}
        onEndEarly={vi.fn()}
        showToast={vi.fn()}
'''
if text.count(old) != 1:
    raise SystemExit(f"tag-scope launch marker count={text.count(old)}")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
