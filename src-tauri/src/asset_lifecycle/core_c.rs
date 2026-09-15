pub(crate) fn finalize_assets_for_owner(
    transaction: &Transaction<'_>,
    paths: &AppPaths,
    asset_ids: &[String],
    draft_id: Option<&str>,
    owner_id: Option<&str>,
) -> Result<DraftFinalization, CommandError> {
    finalize_assets_with_faults(
        transaction,
        paths,
        asset_ids,
        draft_id,
        owner_id,
        PromotionFaults::default(),
    )
}
