from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    source = file.read_text(encoding="utf-8")
    count = source.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one replacement target, found {count}")
    file.write_text(source.replace(old, new, 1), encoding="utf-8")


smoke = Path("tests/webdriver/smoke.py")
source = smoke.read_text(encoding="utf-8")
old_find = '''        except WebDriverProtocolError as error:
            if error.error not in RETRYABLE_FIND_ERRORS:
                raise
            last_error = error
        time.sleep(0.2)
    raise TimeoutError(f"Element was not found: {using}={value}; last error: {last_error}")
'''
new_find = '''        except WebDriverProtocolError as error:
            if error.error not in RETRYABLE_FIND_ERRORS:
                raise
            last_error = error
        except (OSError, json.JSONDecodeError) as error:
            # Element lookup is a read-only W3C operation. A transient HTTP
            # disconnect can be retried safely inside the existing deadline;
            # mutating commands such as clicks and session creation are never
            # retried here because their response could have been lost after
            # the operation was already applied.
            last_error = error
        time.sleep(0.2)
    raise TimeoutError(f"Element was not found: {using}={value}; last error: {last_error}")
'''
if source.count(old_find) != 1:
    raise SystemExit(f"smoke.py: expected one find_element exception block, found {source.count(old_find)}")
source = source.replace(old_find, new_find, 1)

old_finds = '''        except WebDriverProtocolError as error:
            if error.error not in RETRYABLE_FIND_ERRORS:
                raise
            last_error = error
        time.sleep(0.2)
    raise TimeoutError(
        f"Expected at least {minimum} elements for {using}={value}; "
'''
new_finds = '''        except WebDriverProtocolError as error:
            if error.error not in RETRYABLE_FIND_ERRORS:
                raise
            last_error = error
        except (OSError, json.JSONDecodeError) as error:
            last_error = error
        time.sleep(0.2)
    raise TimeoutError(
        f"Expected at least {minimum} elements for {using}={value}; "
'''
if source.count(old_finds) != 1:
    raise SystemExit(f"smoke.py: expected one find_elements exception block, found {source.count(old_finds)}")
source = source.replace(old_finds, new_finds, 1)
smoke.write_text(source, encoding="utf-8")

tests = Path("tests/scripts/test_webdriver_smoke.py")
source = tests.read_text(encoding="utf-8")
replace_import = "import importlib.util\n"
if source.count(replace_import) != 1:
    raise SystemExit("test_webdriver_smoke.py: import insertion point missing")
source = source.replace(replace_import, "import http.client\nimport importlib.util\n", 1)

marker = '''    def test_click_requires_two_stable_hit_tests_before_w3c_click(self) -> None:
'''
test = '''    def test_element_polling_retries_one_transient_remote_disconnect(self) -> None:
        with (
            mock.patch.object(
                smoke,
                "request",
                side_effect=[
                    http.client.RemoteDisconnected("connection closed without response"),
                    {smoke.ELEMENT_KEY: "element-1"},
                ],
            ) as request,
            mock.patch.object(smoke.time, "sleep"),
        ):
            element = smoke.find_element(
                "session-1",
                "css selector",
                "main",
                smoke.deadline_after(1),
            )

        self.assertEqual(element, "element-1")
        self.assertEqual(request.call_count, 2)

    def test_click_requires_two_stable_hit_tests_before_w3c_click(self) -> None:
'''
if source.count(marker) != 1:
    raise SystemExit("test_webdriver_smoke.py: test insertion point missing")
source = source.replace(marker, test, 1)
tests.write_text(source, encoding="utf-8")
