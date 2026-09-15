    fn temp_paths(label: &str) -> AppPaths {
        let root = std::env::temp_dir().join(format!("ostudy-{label}-{}-{}", std::process::id(), unix_time_millis().expect("clock")));
        AppPaths::new(root).expect("paths should initialize")
    }
