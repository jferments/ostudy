use std::path::Path;

pub(crate) const PRODUCTION_APPLICATION_IDENTIFIER: &str = "io.github.jferments.ostudy";
pub(crate) const DEVELOPMENT_APPLICATION_IDENTIFIER: &str = "io.github.jferments.ostudy.dev";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DesktopApplicationProfile {
    Production,
    Development,
}

impl DesktopApplicationProfile {
    pub(crate) fn from_identifier(identifier: &str) -> Result<Self, String> {
        match identifier {
            PRODUCTION_APPLICATION_IDENTIFIER => Ok(Self::Production),
            DEVELOPMENT_APPLICATION_IDENTIFIER => Ok(Self::Development),
            _ => Err(format!(
                "Unsupported desktop application identifier {identifier:?}. Expected {PRODUCTION_APPLICATION_IDENTIFIER:?} or {DEVELOPMENT_APPLICATION_IDENTIFIER:?}."
            )),
        }
    }

    pub(crate) fn identifier(self) -> &'static str {
        match self {
            Self::Production => PRODUCTION_APPLICATION_IDENTIFIER,
            Self::Development => DEVELOPMENT_APPLICATION_IDENTIFIER,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Development => "development",
        }
    }

    pub(crate) fn window_title(self) -> &'static str {
        match self {
            Self::Production => "ostudy",
            Self::Development => "ostudy Dev",
        }
    }
}

pub(crate) fn validate_desktop_runtime(
    identifier: &str,
    app_data_dir: &Path,
    debug_build: bool,
) -> Result<DesktopApplicationProfile, String> {
    let profile = DesktopApplicationProfile::from_identifier(identifier)?;

    match (debug_build, profile) {
        (true, DesktopApplicationProfile::Production) => {
            return Err(
                "A desktop debug/development build resolved the production oStudy identifier. Startup was refused before opening the learner library. Use the maintained development wrapper so the build uses io.github.jferments.ostudy.dev."
                    .to_string(),
            );
        }
        (false, DesktopApplicationProfile::Development) => {
            return Err(
                "A desktop release build resolved the development oStudy identifier. Startup was refused because release builds must use io.github.jferments.ostudy."
                    .to_string(),
            );
        }
        _ => {}
    }

    let resolved_identifier = app_data_dir
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            format!(
                "Could not verify the oStudy application-data directory for the {} profile.",
                profile.as_str()
            )
        })?;

    if resolved_identifier != profile.identifier() {
        return Err(format!(
            "The {} oStudy profile resolved application data under {resolved_identifier:?} instead of {:?}. Startup was refused before opening the learner library.",
            profile.as_str(),
            profile.identifier(),
        ));
    }

    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_debug_profile_has_its_own_data_directory() {
        let profile = validate_desktop_runtime(
            DEVELOPMENT_APPLICATION_IDENTIFIER,
            Path::new("/tmp/xdg/io.github.jferments.ostudy.dev"),
            true,
        )
        .expect("development profile should be admitted");
        assert_eq!(profile, DesktopApplicationProfile::Development);
    }

    #[test]
    fn production_release_profile_has_its_own_data_directory() {
        let profile = validate_desktop_runtime(
            PRODUCTION_APPLICATION_IDENTIFIER,
            Path::new("/tmp/xdg/io.github.jferments.ostudy"),
            false,
        )
        .expect("production profile should be admitted");
        assert_eq!(profile, DesktopApplicationProfile::Production);
    }

    #[test]
    fn debug_build_cannot_open_the_production_profile() {
        let error = validate_desktop_runtime(
            PRODUCTION_APPLICATION_IDENTIFIER,
            Path::new("/tmp/xdg/io.github.jferments.ostudy"),
            true,
        )
        .expect_err("debug build must fail closed");
        assert!(error.contains("refused before opening the learner library"));
    }

    #[test]
    fn development_profile_cannot_resolve_the_production_data_directory() {
        let error = validate_desktop_runtime(
            DEVELOPMENT_APPLICATION_IDENTIFIER,
            Path::new("/tmp/xdg/io.github.jferments.ostudy"),
            true,
        )
        .expect_err("development profile must not alias production storage");
        assert!(error.contains("instead of"));
    }

    #[test]
    fn release_build_cannot_use_the_development_profile() {
        let error = validate_desktop_runtime(
            DEVELOPMENT_APPLICATION_IDENTIFIER,
            Path::new("/tmp/xdg/io.github.jferments.ostudy.dev"),
            false,
        )
        .expect_err("release build must use production identity");
        assert!(error.contains("release builds must use"));
    }
}
