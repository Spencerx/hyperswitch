use std::marker::PhantomData;

use api_models::enums::FrmSuggestion;
use async_trait::async_trait;
use common_utils::ext_traits::AsyncExt;
use error_stack::ResultExt;
use router_derive;
use router_env::{instrument, tracing};

use super::{BoxedOperation, Domain, GetTracker, Operation, UpdateTracker, ValidateRequest};
use crate::{
    core::{
        errors::{self, RouterResult, StorageErrorExt},
        payments::{helpers, operations, PaymentData},
    },
    events::audit_events::{AuditEvent, AuditEventType},
    routes::{app::ReqState, SessionState},
    services,
    types::{
        self as core_types,
        api::{self, PaymentIdTypeExt},
        domain,
        storage::{self, enums},
    },
    utils::OptionExt,
};

#[derive(Debug, Clone, Copy, router_derive::PaymentOperation)]
#[operation(operations = "all", flow = "cancel")]
pub struct PaymentCancel;

type PaymentCancelOperation<'b, F> =
    BoxedOperation<'b, F, api::PaymentsCancelRequest, PaymentData<F>>;

#[async_trait]
impl<F: Send + Clone + Sync> GetTracker<F, PaymentData<F>, api::PaymentsCancelRequest>
    for PaymentCancel
{
    #[instrument(skip_all)]
    async fn get_trackers<'a>(
        &'a self,
        state: &'a SessionState,
        payment_id: &api::PaymentIdType,
        request: &api::PaymentsCancelRequest,
        merchant_context: &domain::MerchantContext,
        _auth_flow: services::AuthFlow,
        _header_payload: &hyperswitch_domain_models::payments::HeaderPayload,
    ) -> RouterResult<
        operations::GetTrackerResponse<'a, F, api::PaymentsCancelRequest, PaymentData<F>>,
    > {
        let db = &*state.store;
        let key_manager_state = &state.into();

        let merchant_id = merchant_context.get_merchant_account().get_id();
        let storage_scheme = merchant_context.get_merchant_account().storage_scheme;
        let payment_id = payment_id
            .get_payment_intent_id()
            .change_context(errors::ApiErrorResponse::PaymentNotFound)?;

        let payment_intent = db
            .find_payment_intent_by_payment_id_merchant_id(
                key_manager_state,
                &payment_id,
                merchant_id,
                merchant_context.get_merchant_key_store(),
                storage_scheme,
            )
            .await
            .to_not_found_response(errors::ApiErrorResponse::PaymentNotFound)?;

        helpers::validate_payment_status_against_not_allowed_statuses(
            payment_intent.status,
            &[
                enums::IntentStatus::Failed,
                enums::IntentStatus::Succeeded,
                enums::IntentStatus::Cancelled,
                enums::IntentStatus::Processing,
                enums::IntentStatus::RequiresMerchantAction,
            ],
            "cancel",
        )?;

        let mut payment_attempt = db
            .find_payment_attempt_by_payment_id_merchant_id_attempt_id(
                &payment_intent.payment_id,
                merchant_id,
                payment_intent.active_attempt.get_id().as_str(),
                storage_scheme,
            )
            .await
            .to_not_found_response(errors::ApiErrorResponse::PaymentNotFound)?;

        let shipping_address = helpers::get_address_by_id(
            state,
            payment_intent.shipping_address_id.clone(),
            merchant_context.get_merchant_key_store(),
            &payment_intent.payment_id,
            merchant_id,
            merchant_context.get_merchant_account().storage_scheme,
        )
        .await?;

        let billing_address = helpers::get_address_by_id(
            state,
            payment_intent.billing_address_id.clone(),
            merchant_context.get_merchant_key_store(),
            &payment_intent.payment_id,
            merchant_id,
            merchant_context.get_merchant_account().storage_scheme,
        )
        .await?;

        let payment_method_billing = helpers::get_address_by_id(
            state,
            payment_attempt.payment_method_billing_address_id.clone(),
            merchant_context.get_merchant_key_store(),
            &payment_intent.payment_id,
            merchant_id,
            merchant_context.get_merchant_account().storage_scheme,
        )
        .await?;

        let currency = payment_attempt.currency.get_required_value("currency")?;
        let amount = payment_attempt.get_total_amount().into();

        payment_attempt
            .cancellation_reason
            .clone_from(&request.cancellation_reason);

        let creds_identifier = request
            .merchant_connector_details
            .as_ref()
            .map(|mcd| mcd.creds_identifier.to_owned());
        request
            .merchant_connector_details
            .to_owned()
            .async_map(|mcd| async {
                helpers::insert_merchant_connector_creds_to_config(
                    db,
                    merchant_context.get_merchant_account().get_id(),
                    mcd,
                )
                .await
            })
            .await
            .transpose()?;

        let profile_id = payment_intent
            .profile_id
            .as_ref()
            .get_required_value("profile_id")
            .change_context(errors::ApiErrorResponse::InternalServerError)
            .attach_printable("'profile_id' not set in payment intent")?;

        let business_profile = db
            .find_business_profile_by_profile_id(
                key_manager_state,
                merchant_context.get_merchant_key_store(),
                profile_id,
            )
            .await
            .to_not_found_response(errors::ApiErrorResponse::ProfileNotFound {
                id: profile_id.get_string_repr().to_owned(),
            })?;

        let payment_data = PaymentData {
            flow: PhantomData,
            payment_intent,
            payment_attempt,
            currency,
            amount,
            email: None,
            mandate_id: None,
            mandate_connector: None,
            setup_mandate: None,
            customer_acceptance: None,
            token: None,
            token_data: None,
            address: core_types::PaymentAddress::new(
                shipping_address.as_ref().map(From::from),
                billing_address.as_ref().map(From::from),
                payment_method_billing.as_ref().map(From::from),
                business_profile.use_billing_as_payment_method_billing,
            ),
            confirm: None,
            payment_method_data: None,
            payment_method_token: None,
            payment_method_info: None,
            force_sync: None,
            all_keys_required: None,
            refunds: vec![],
            disputes: vec![],
            attempts: None,
            sessions_token: vec![],
            card_cvc: None,
            creds_identifier,
            pm_token: None,
            connector_customer_id: None,
            recurring_mandate_payment_data: None,
            ephemeral_key: None,
            multiple_capture_data: None,
            redirect_response: None,
            surcharge_details: None,
            frm_message: None,
            payment_link_data: None,
            incremental_authorization_details: None,
            authorizations: vec![],
            authentication: None,
            recurring_details: None,
            poll_config: None,
            tax_data: None,
            session_id: None,
            service_details: None,
            card_testing_guard_data: None,
            vault_operation: None,
            threeds_method_comp_ind: None,
            whole_connector_response: None,
        };

        let get_trackers_response = operations::GetTrackerResponse {
            operation: Box::new(self),
            customer_details: None,
            payment_data,
            business_profile,
            mandate_type: None,
        };

        Ok(get_trackers_response)
    }
}

#[async_trait]
impl<F: Clone + Sync> UpdateTracker<F, PaymentData<F>, api::PaymentsCancelRequest>
    for PaymentCancel
{
    #[instrument(skip_all)]
    async fn update_trackers<'b>(
        &'b self,
        state: &'b SessionState,
        req_state: ReqState,
        mut payment_data: PaymentData<F>,
        _customer: Option<domain::Customer>,
        storage_scheme: enums::MerchantStorageScheme,
        _updated_customer: Option<storage::CustomerUpdate>,
        key_store: &domain::MerchantKeyStore,
        _frm_suggestion: Option<FrmSuggestion>,
        _header_payload: hyperswitch_domain_models::payments::HeaderPayload,
    ) -> RouterResult<(PaymentCancelOperation<'b, F>, PaymentData<F>)>
    where
        F: 'b + Send,
    {
        let cancellation_reason = payment_data.payment_attempt.cancellation_reason.clone();
        let (intent_status_update, attempt_status_update) =
            if payment_data.payment_intent.status != enums::IntentStatus::RequiresCapture {
                let payment_intent_update = storage::PaymentIntentUpdate::PGStatusUpdate {
                    status: enums::IntentStatus::Cancelled,
                    updated_by: storage_scheme.to_string(),
                    incremental_authorization_allowed: None,
                };
                (Some(payment_intent_update), enums::AttemptStatus::Voided)
            } else {
                (None, enums::AttemptStatus::VoidInitiated)
            };

        if let Some(payment_intent_update) = intent_status_update {
            payment_data.payment_intent = state
                .store
                .update_payment_intent(
                    &state.into(),
                    payment_data.payment_intent,
                    payment_intent_update,
                    key_store,
                    storage_scheme,
                )
                .await
                .to_not_found_response(errors::ApiErrorResponse::PaymentNotFound)?;
        }

        state
            .store
            .update_payment_attempt_with_attempt_id(
                payment_data.payment_attempt.clone(),
                storage::PaymentAttemptUpdate::VoidUpdate {
                    status: attempt_status_update,
                    cancellation_reason: cancellation_reason.clone(),
                    updated_by: storage_scheme.to_string(),
                },
                storage_scheme,
            )
            .await
            .to_not_found_response(errors::ApiErrorResponse::PaymentNotFound)?;
        req_state
            .event_context
            .event(AuditEvent::new(AuditEventType::PaymentCancelled {
                cancellation_reason,
            }))
            .with(payment_data.to_event())
            .emit();
        Ok((Box::new(self), payment_data))
    }
}

impl<F: Send + Clone + Sync> ValidateRequest<F, api::PaymentsCancelRequest, PaymentData<F>>
    for PaymentCancel
{
    #[instrument(skip_all)]
    fn validate_request<'a, 'b>(
        &'b self,
        request: &api::PaymentsCancelRequest,
        merchant_context: &'a domain::MerchantContext,
    ) -> RouterResult<(PaymentCancelOperation<'b, F>, operations::ValidateResult)> {
        Ok((
            Box::new(self),
            operations::ValidateResult {
                merchant_id: merchant_context.get_merchant_account().get_id().to_owned(),
                payment_id: api::PaymentIdType::PaymentIntentId(request.payment_id.to_owned()),
                storage_scheme: merchant_context.get_merchant_account().storage_scheme,
                requeue: false,
            },
        ))
    }
}
