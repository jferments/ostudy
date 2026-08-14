fn main() {
    // JavaScript uses Tauri's mobile-plugin event channel for lifecycle, Back,
    // insets, network, and incoming-share observations. Listing the listener
    // commands here generates the allow-register-listener / allow-remove-listener
    // ACL entries referenced by permissions/default.toml.
    tauri_plugin::Builder::new(&["register_listener", "remove_listener"])
        .android_path("android")
        .build();
}
