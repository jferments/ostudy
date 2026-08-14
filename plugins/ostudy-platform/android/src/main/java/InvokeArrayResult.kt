package io.github.jferments.ostudy.platform

import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject

/**
 * Tauri 2.11's Android Invoke transport resolves JSON objects, not a top-level
 * JSON array. Keep the plugin command ergonomic while preserving an explicit
 * wire envelope that Rust can deserialize and unwrap.
 */
internal fun Invoke.resolve(result: JSArray) {
    val envelope = JSObject()
    envelope.put("items", result)
    resolve(envelope)
}
