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
text = text.replace(old, new, 1)

old = '''            match page.text() {
                Ok(text) => extracted.push((page_number, text.all())),
                Err(error) => crate::logging::info(
                    "pdfium_native_text_page_unavailable",
                    format!(
                        "page={page_number} detail={}",
                        bounded_detail(&error.to_string(), 300)
                    ),
                ),
            }
        }
'''
new = '''            match page.text() {
                Ok(text) => extracted.push((page_number, text.all())),
                Err(error) => crate::logging::info(
                    "pdfium_native_text_page_unavailable",
                    format!(
                        "page={page_number} detail={}",
                        bounded_detail(&error.to_string(), 300)
                    ),
                ),
            };
        }
'''
if text.count(old) != 1:
    raise SystemExit(f"PDFium lifetime template marker count={text.count(old)}")
text = text.replace(old, new, 1)
path.write_text(text, encoding="utf-8")
