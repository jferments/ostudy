from pathlib import Path

path = Path(".github/pr41-followup-v7.py")
text = path.read_text(encoding="utf-8")
old = """old = '''    pendingStartRef.current = null;
    pendingReviewRef.current = null;
    setSession(null);
'''
new = '''    pendingStartRef.current = null;
    pendingReviewRef.current = null;
    setReturnTarget(launchRequest.returnTarget);
    setSession(null);
'''
"""
new = """old = '''    handledLaunchRequestRef.current = launchRequest.requestId;
    pendingStartRef.current = null;
    pendingReviewRef.current = null;
    setSession(null);
'''
new = '''    handledLaunchRequestRef.current = launchRequest.requestId;
    pendingStartRef.current = null;
    pendingReviewRef.current = null;
    setReturnTarget(launchRequest.returnTarget);
    setSession(null);
'''
"""
if text.count(old) != 1:
    raise SystemExit(f"main applicator launch-template marker count={text.count(old)}")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
