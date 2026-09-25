use super::*;

// Keep the original strict suite unchanged and run it through the Fast filter.
mod document_validated {
    use super::ValidatedPayload as TypedPayload;
    include!("validated_tests.rs");
}
