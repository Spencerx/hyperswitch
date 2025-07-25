use common_utils::{
    consts::DEFAULT_LOCALE,
    ext_traits::{OptionExt, ValueExt},
};
use scheduler::{
    consumer::{self, workflows::ProcessTrackerWorkflow},
    errors,
};

use crate::{
    core::payouts,
    errors as core_errors,
    routes::SessionState,
    types::{api, domain, storage},
};

pub struct AttachPayoutAccountWorkflow;

#[async_trait::async_trait]
impl ProcessTrackerWorkflow<SessionState> for AttachPayoutAccountWorkflow {
    async fn execute_workflow<'a>(
        &'a self,
        state: &'a SessionState,
        process: storage::ProcessTracker,
    ) -> Result<(), errors::ProcessTrackerError> {
        // Gather context
        let db = &*state.store;
        let tracking_data: api::PayoutRetrieveRequest = process
            .tracking_data
            .clone()
            .parse_value("PayoutRetrieveRequest")?;

        let merchant_id = tracking_data
            .merchant_id
            .clone()
            .get_required_value("merchant_id")?;
        let key_manager_state = &state.into();
        let key_store = db
            .get_merchant_key_store_by_merchant_id(
                key_manager_state,
                &merchant_id,
                &db.get_master_key().to_vec().into(),
            )
            .await?;

        let merchant_account = db
            .find_merchant_account_by_merchant_id(key_manager_state, &merchant_id, &key_store)
            .await?;

        let request = api::payouts::PayoutRequest::PayoutRetrieveRequest(tracking_data);

        let merchant_context = domain::MerchantContext::NormalMerchant(Box::new(domain::Context(
            merchant_account.clone(),
            key_store.clone(),
        )));
        let mut payout_data = Box::pin(payouts::make_payout_data(
            state,
            &merchant_context,
            None,
            &request,
            DEFAULT_LOCALE,
        ))
        .await?;

        payouts::payouts_core(state, &merchant_context, &mut payout_data, None, None).await?;

        Ok(())
    }

    async fn error_handler<'a>(
        &'a self,
        state: &'a SessionState,
        process: storage::ProcessTracker,
        error: errors::ProcessTrackerError,
    ) -> core_errors::CustomResult<(), errors::ProcessTrackerError> {
        consumer::consumer_error_handler(state.store.as_scheduler(), process, error).await
    }
}
